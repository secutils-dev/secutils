use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::{
            export::{ExportedResponder, ExportedResponderRequest},
            import::{
                ConflictResolution, ImportEntityResult, ImportEntitySelection, resolve_name,
                should_skip,
            },
        },
    },
    utils::webhooks::ResponderRequest,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};
use time::OffsetDateTime;
use tracing::warn;
use uuid::Uuid;

pub async fn import_responders<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    responders: &[ExportedResponder],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing responders once for overwritten resolution.
    let existing_responders = api
        .webhooks(user)
        .get_responders()
        .await
        .unwrap_or_default();
    let mut used_names: HashSet<_> = existing_responders.iter().map(|r| r.name.clone()).collect();

    for exported in responders {
        let resp = &exported.responder;
        let selection = selections.get(&resp.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let name = resolve_name(&resp.name, selection, &used_names);

        // Handle overwrite.
        if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite))
            && let Some(e) = existing_responders.iter().find(|r| r.name == resp.name)
        {
            let _ = api.webhooks(user).remove_responder(e.id).await;
            used_names.remove(&resp.name);
        }

        // Clone the responder and assign new ID and timestamps.
        let mut new_responder = resp.clone();
        let new_id = Uuid::now_v7();
        new_responder.id = new_id;
        new_responder.name = name.clone();
        let now = OffsetDateTime::now_utc();
        new_responder.created_at = now;
        new_responder.updated_at = now;

        match api
            .db
            .webhooks()
            .insert_responder(user.id, &new_responder)
            .await
        {
            Ok(_) => {
                used_names.insert(name);
                result.imported += 1;

                // Import history if available.
                if !exported.history.is_empty() {
                    import_responder_history(api, user, new_id, &exported.history).await;
                }
            }
            Err(err) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("Responder '{}': {err}", resp.name));
            }
        }
    }
    result
}

async fn import_responder_history<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    new_responder_id: Uuid,
    history: &[ExportedResponderRequest],
) {
    for entry in history {
        // Parse headers from JSON object format: { "key": "value" } → Vec<(Cow, Cow)>.
        let headers = entry.headers.as_ref().and_then(|h| {
            h.as_object().map(|obj| {
                obj.iter()
                    .map(|(k, v)| {
                        (
                            Cow::Owned(k.clone()),
                            Cow::Owned(v.as_str().unwrap_or("").as_bytes().to_vec()),
                        )
                    })
                    .collect::<Vec<_>>()
            })
        });
        let body = entry
            .body
            .as_ref()
            .map(|s| Cow::Owned(s.as_bytes().to_vec()));
        let response_headers = entry.response_headers.as_ref().and_then(|h| {
            h.as_object().map(|obj| {
                obj.iter()
                    .map(|(k, v)| {
                        (
                            Cow::Owned(k.clone()),
                            Cow::Owned(v.as_str().unwrap_or("").as_bytes().to_vec()),
                        )
                    })
                    .collect::<Vec<_>>()
            })
        });
        let response_body = entry
            .response_body
            .as_ref()
            .map(|s| Cow::Owned(s.as_bytes().to_vec()));
        let client_address = entry.client_address.as_ref().and_then(|s| s.parse().ok());

        let request = ResponderRequest {
            id: Uuid::now_v7(),
            responder_id: new_responder_id,
            client_address,
            method: Cow::Owned(entry.method.clone()),
            headers,
            url: Cow::Owned(entry.url.clone()),
            body,
            created_at: entry.created_at,
            duration_ms: entry.duration_ms,
            response_status_code: entry.response_status_code,
            response_headers,
            response_body,
        };

        if let Err(err) = api
            .db
            .webhooks()
            .insert_responder_request(user.id, &request)
            .await
        {
            warn!(
                responder.id = %new_responder_id,
                request.id = %entry.id,
                "Failed to import responder history entry: {err:?}",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        file::{UserDataImportFile, UserDataImportFileData},
        params::{
            ImportAction, ImportEntitySelection, ImportMode, ImportSelections, UserDataImportParams,
        },
    };
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::user_data::{export::ExportedResponder, import::execute_import},
        utils::webhooks::{
            Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
        },
    };
    use sqlx::PgPool;
    use time::macros::datetime;
    use uuid::Uuid;

    fn minimal_responder(id: Uuid, name: &str) -> ExportedResponder {
        ExportedResponder {
            responder: Responder {
                id,
                name: name.to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/test".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 1,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: crate::users::SecretsAccess::None,
                },
                created_at: datetime!(2020-01-01 00:00:00 UTC),
                updated_at: datetime!(2020-01-01 00:00:00 UTC),
            },
            history: vec![],
        }
    }

    fn make_responders_file(responders: Vec<ExportedResponder>) -> UserDataImportFile {
        UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts: vec![],
                secrets: vec![],
                responders,
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
            },
        }
    }

    #[sqlx::test]
    async fn import_responders_merge(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let responder_id = Uuid::now_v7();
        let file = make_responders_file(vec![minimal_responder(responder_id, "my_responder")]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                responders: vec![ImportEntitySelection {
                    source_id: responder_id,
                    action: ImportAction::Import,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.responders.imported, 1);
        assert_eq!(result.results.responders.failed, 0);

        let responders = api.webhooks(&user).get_responders().await?;
        assert_eq!(responders.len(), 1);
        assert_eq!(responders[0].name, "my_responder");

        Ok(())
    }
}
