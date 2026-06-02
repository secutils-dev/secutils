use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::{
            export::{ExportedResponder, ExportedResponderRequest},
            import::{
                ConflictResolution, ImportEntityResult, ImportEntitySelection, remap_tag_ids,
                resolve_name, should_skip,
            },
        },
    },
    utils::webhooks::{ResponderMethod, ResponderRequest, RespondersCreateParams},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};
use tracing::warn;
use uuid::Uuid;

pub async fn import_responders<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    responders: &[ExportedResponder],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
    tag_id_map: &HashMap<Uuid, Uuid>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing responders once for overwrite resolution.
    let webhooks_api = api.webhooks(user);
    let existing_responders = webhooks_api.get_responders().await.unwrap_or_default();
    let mut used_names: HashSet<_> = existing_responders.iter().map(|r| r.name.clone()).collect();
    let mut deleted_ids: HashSet<Uuid> = HashSet::new();

    for exported in responders {
        let resp = &exported.responder;
        let selection = selections.get(&resp.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let location = &resp.location;
        let name = resolve_name(&resp.name, selection, &used_names);
        let is_overwrite =
            selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite));

        if is_overwrite {
            // Delete an existing responder with the same name.
            if let Some(e) = existing_responders
                .iter()
                .find(|r| r.name == resp.name && !deleted_ids.contains(&r.id))
                && deleted_ids.insert(e.id)
            {
                let _ = webhooks_api.remove_responder(e.id).await;
                used_names.remove(&e.name);
            }

            // Delete any existing responder that conflicts on location+method.
            let import_loc = location.to_string();
            if let Some(e) = existing_responders.iter().find(|r| {
                !deleted_ids.contains(&r.id)
                    && r.location.to_string() == import_loc
                    && (r.method == resp.method
                        || r.method == ResponderMethod::Any
                        || resp.method == ResponderMethod::Any)
            }) && deleted_ids.insert(e.id)
            {
                let _ = webhooks_api.remove_responder(e.id).await;
                used_names.remove(&e.name);
            }
        }

        match webhooks_api
            .create_responder(RespondersCreateParams {
                name: name.clone(),
                location: location.clone(),
                method: resp.method,
                enabled: resp.enabled,
                settings: resp.settings.clone(),
                tag_ids: remap_tag_ids(&resp.tags, tag_id_map),
            })
            .await
        {
            Ok(new_responder) => {
                used_names.insert(name);
                result.imported += 1;

                // Import history if available.
                if !exported.history.is_empty() {
                    import_responder_history(api, user, new_responder.id, &exported.history).await;
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
    let webhooks_db = api.db.webhooks();
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

        if let Err(err) = webhooks_db
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
        responder_with_location(id, name, "/test", ResponderMethod::Get)
    }

    fn responder_with_location(
        id: Uuid,
        name: &str,
        path: &str,
        method: ResponderMethod,
    ) -> ExportedResponder {
        ExportedResponder {
            responder: Responder {
                id,
                name: name.to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: path.to_string(),
                    subdomain_prefix: None,
                },
                method,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 1,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: crate::users::SecretsAccess::None,
                    notifications: None,
                },
                tags: vec![],
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
                tags: vec![],
                scripts: vec![],
                secrets: vec![],
                responders,
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
                settings: None,
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

    #[sqlx::test]
    async fn import_responders_overwrite_resolves_location_conflict(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Create an existing responder with a specific location+method.
        let existing_id = Uuid::now_v7();
        let existing = Responder {
            id: existing_id,
            name: "old-name".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/my-path".to_string(),
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
                notifications: None,
            },
            tags: vec![],
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        };
        api.db
            .webhooks()
            .insert_responder(user.id, &existing)
            .await?;

        // Import a responder with a different name but the same location+method.
        let import_id = Uuid::now_v7();
        let file = make_responders_file(vec![responder_with_location(
            import_id,
            "new-name",
            "/my-path",
            ResponderMethod::Get,
        )]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                responders: vec![ImportEntitySelection {
                    source_id: import_id,
                    action: ImportAction::Import,
                    conflict_resolution: Some(
                        super::super::super::params::ConflictResolution::Overwrite,
                    ),
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.responders.imported, 1);
        assert_eq!(result.results.responders.failed, 0);

        // The old responder should be deleted and replaced by the new one.
        let responders = api.webhooks(&user).get_responders().await?;
        assert_eq!(responders.len(), 1);
        assert_eq!(responders[0].name, "new-name");

        Ok(())
    }
}
