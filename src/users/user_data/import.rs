mod detect_conflicts;
mod detect_deletions;
mod detect_duplicates;
mod file;
mod importers;
mod params;
mod resolve_name;
mod results;
mod should_skip;

pub use self::{
    file::ImportedScript,
    params::{
        ConflictResolution, ImportAction, ImportEntitySelection, ImportMode, UserDataImportParams,
        UserDataImportPreviewParams,
    },
    resolve_name::resolve_name,
    results::ImportEntityResult,
    should_skip::should_skip,
};

use super::shared::DATA_FILE_VERSION;
use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::User,
};
use detect_conflicts::detect_conflicts;
use detect_deletions::detect_deletions;
use detect_duplicates::detect_intra_file_duplicates;
use importers::{
    certificate_templates::import_certificate_templates,
    content_security_policies::import_content_security_policies,
    private_keys::import_private_keys,
    responders::import_responders,
    scripts::import_scripts,
    secrets::import_secrets,
    trackers::{TrackerKind, import_trackers},
};
use results::{
    ApplyDeleteItem, ApplyDeleteSummary, ImportEntitySummary, ImportPreviewSummary,
    ImportResultsSummary, UserDataImportPreview, UserDataImportResult,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Generates a preview of what would happen during import.
pub async fn generate_import_preview<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    params: &UserDataImportPreviewParams,
) -> anyhow::Result<UserDataImportPreview> {
    let file = &params.data;
    let mut warnings = Vec::new();

    if file.version != DATA_FILE_VERSION {
        return Ok(UserDataImportPreview {
            valid: false,
            version: file.version,
            summary: ImportPreviewSummary::default(),
            warnings: vec![format!(
                "Unsupported export file version: {}. Expected version {DATA_FILE_VERSION}.",
                file.version
            )],
            to_delete: None,
        });
    }

    // Check for duplicate names within the import file itself.
    detect_intra_file_duplicates(&file.data, &mut warnings);

    let is_apply = params.mode == ImportMode::Apply;

    // Extract (id, name) pairs from import file for each entity type.
    let import_scripts: Vec<(Uuid, &str)> = file
        .data
        .scripts
        .iter()
        .map(|s| (s.id, s.name.as_str()))
        .collect();
    let import_secrets: Vec<(Uuid, &str)> = file
        .data
        .secrets
        .iter()
        .map(|s| (s.id, s.name.as_str()))
        .collect();
    let import_responders: Vec<(Uuid, &str)> = file
        .data
        .responders
        .iter()
        .map(|r| (r.responder.id, r.responder.name.as_str()))
        .collect();
    let import_templates: Vec<(Uuid, &str)> = file
        .data
        .certificate_templates
        .iter()
        .map(|t| (t.id, t.name.as_str()))
        .collect();
    let import_keys: Vec<(Uuid, &str)> = file
        .data
        .private_keys
        .iter()
        .map(|k| (k.id, k.name.as_str()))
        .collect();
    let import_csps: Vec<(Uuid, &str)> = file
        .data
        .content_security_policies
        .iter()
        .map(|c| (c.id, c.name.as_str()))
        .collect();
    let import_page_trackers: Vec<(Uuid, &str)> = file
        .data
        .page_trackers
        .iter()
        .map(|t| (t.id, t.name.as_str()))
        .collect();
    let import_api_trackers: Vec<(Uuid, &str)> = file
        .data
        .api_trackers
        .iter()
        .map(|t| (t.id, t.name.as_str()))
        .collect();

    // Fetch existing data only when needed: conflict detection requires imports to be non-empty,
    // while Apply mode always needs existing data for deletion detection.
    let existing_scripts = fetch_existing(!import_scripts.is_empty() || is_apply, || async {
        Ok(api
            .scripts(user)
            .list_scripts(None)
            .await?
            .into_iter()
            .map(|s| (s.id, s.name))
            .collect())
    })
    .await?;

    let existing_secrets = fetch_existing(!import_secrets.is_empty() || is_apply, || async {
        Ok(api
            .secrets(user)
            .list_secrets()
            .await?
            .into_iter()
            .map(|s| (s.id, s.name))
            .collect())
    })
    .await?;

    let existing_responders = fetch_existing(!import_responders.is_empty() || is_apply, || async {
        Ok(api
            .webhooks(user)
            .get_responders()
            .await?
            .into_iter()
            .map(|r| (r.id, r.name))
            .collect())
    })
    .await?;

    let existing_templates = fetch_existing(!import_templates.is_empty() || is_apply, || async {
        Ok(api
            .certificates()
            .get_certificate_templates(user.id)
            .await?
            .into_iter()
            .map(|t| (t.id, t.name))
            .collect())
    })
    .await?;

    let existing_keys = fetch_existing(!import_keys.is_empty() || is_apply, || async {
        Ok(api
            .certificates()
            .get_private_keys(user.id)
            .await?
            .into_iter()
            .map(|k| (k.id, k.name))
            .collect())
    })
    .await?;

    let existing_csps = fetch_existing(!import_csps.is_empty() || is_apply, || async {
        Ok(api
            .web_security()
            .get_content_security_policies(user.id)
            .await?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect())
    })
    .await?;

    let existing_page_trackers =
        fetch_existing(!import_page_trackers.is_empty() || is_apply, || async {
            Ok(api
                .web_scraping(user)
                .get_page_trackers()
                .await?
                .into_iter()
                .map(|t| (t.id, t.name))
                .collect())
        })
        .await?;

    let existing_api_trackers =
        fetch_existing(!import_api_trackers.is_empty() || is_apply, || async {
            Ok(api
                .web_scraping(user)
                .get_api_trackers()
                .await?
                .into_iter()
                .map(|t| (t.id, t.name))
                .collect())
        })
        .await?;

    // Compute summaries and deletion candidates for each entity type.
    let (scripts_summary, scripts_deletions) =
        entity_preview(&import_scripts, &existing_scripts, is_apply);
    let (secrets_summary, _) = entity_preview(&import_secrets, &existing_secrets, is_apply);
    let (responders_summary, responders_deletions) =
        entity_preview(&import_responders, &existing_responders, is_apply);
    let (templates_summary, templates_deletions) =
        entity_preview(&import_templates, &existing_templates, is_apply);
    let (keys_summary, keys_deletions) = entity_preview(&import_keys, &existing_keys, is_apply);
    let (csps_summary, csps_deletions) = entity_preview(&import_csps, &existing_csps, is_apply);
    let (page_trackers_summary, page_trackers_deletions) =
        entity_preview(&import_page_trackers, &existing_page_trackers, is_apply);
    let (api_trackers_summary, api_trackers_deletions) =
        entity_preview(&import_api_trackers, &existing_api_trackers, is_apply);

    // Secrets deletion has special rules: only detect when encrypted values are present.
    let secrets_deletions = if is_apply {
        let has_encrypted_secrets = file.secrets_encryption.is_some();
        if has_encrypted_secrets && !import_secrets.is_empty() {
            entity_preview(&import_secrets, &existing_secrets, true).1
        } else {
            if !existing_secrets.is_empty() && import_secrets.is_empty() {
                warnings.push(
                    "Secret values not included in import file - existing secrets will not be deleted."
                        .to_string(),
                );
            } else if !import_secrets.is_empty() {
                warnings.push(
                    "Secret names are included but values are not encrypted - existing secret values will be preserved, no secrets will be deleted."
                        .to_string(),
                );
            }
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let to_delete = is_apply.then_some(ApplyDeleteSummary {
        scripts: scripts_deletions,
        secrets: secrets_deletions,
        responders: responders_deletions,
        certificate_templates: templates_deletions,
        private_keys: keys_deletions,
        content_security_policies: csps_deletions,
        page_trackers: page_trackers_deletions,
        api_trackers: api_trackers_deletions,
    });

    Ok(UserDataImportPreview {
        valid: true,
        version: file.version,
        summary: ImportPreviewSummary {
            scripts: scripts_summary,
            secrets: secrets_summary,
            responders: responders_summary,
            certificate_templates: templates_summary,
            private_keys: keys_summary,
            content_security_policies: csps_summary,
            page_trackers: page_trackers_summary,
            api_trackers: api_trackers_summary,
        },
        warnings,
        to_delete,
    })
}

/// Conditionally fetches existing entities. Returns an empty list when `needed` is false,
/// avoiding unnecessary DB/API calls (e.g., when the import file has no entities of this type
/// and we're not in Apply mode where deletions must be detected).
async fn fetch_existing<F, Fut>(needed: bool, fetch: F) -> anyhow::Result<Vec<(Uuid, String)>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<Vec<(Uuid, String)>>>,
{
    if needed {
        fetch().await
    } else {
        Ok(Vec::new())
    }
}

/// Computes the preview summary and deletion candidates for a single entity type.
fn entity_preview(
    import_items: &[(Uuid, &str)],
    existing_items: &[(Uuid, String)],
    is_apply: bool,
) -> (ImportEntitySummary, Vec<ApplyDeleteItem>) {
    let existing_refs: Vec<(Uuid, &str)> = existing_items
        .iter()
        .map(|(id, name)| (*id, name.as_str()))
        .collect();
    let summary = ImportEntitySummary {
        total: import_items.len(),
        conflicts: detect_conflicts(import_items, &existing_refs),
    };
    let deletions = if is_apply {
        let import_names: Vec<&str> = import_items.iter().map(|(_, n)| *n).collect();
        detect_deletions(&import_names, &existing_refs)
    } else {
        Vec::new()
    };
    (summary, deletions)
}

/// Executes the import with the specified selections and conflict resolution.
pub async fn execute_import<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    params: UserDataImportParams,
) -> anyhow::Result<UserDataImportResult> {
    let file = params.data;

    if file.version != DATA_FILE_VERSION {
        anyhow::bail!("Unsupported export file version: {}", file.version);
    }

    // Build selection maps for quick lookup.
    macro_rules! selection_map {
        ($field:expr) => {
            $field
                .iter()
                .map(|s| (s.source_id, s))
                .collect::<HashMap<Uuid, &params::ImportEntitySelection>>()
        };
    }
    let scripts_selections = selection_map!(params.selections.scripts);
    let secrets_selections = selection_map!(params.selections.secrets);
    let responders_selections = selection_map!(params.selections.responders);
    let templates_selections = selection_map!(params.selections.certificate_templates);
    let keys_selections = selection_map!(params.selections.private_keys);
    let csps_selections = selection_map!(params.selections.content_security_policies);
    let page_trackers_selections = selection_map!(params.selections.page_trackers);
    let api_trackers_selections = selection_map!(params.selections.api_trackers);

    // 1. Import scripts.
    let mut scripts_result =
        import_scripts(api, user, &file.data.scripts, &scripts_selections).await;

    // 2. Import secrets.
    let mut secrets_result = import_secrets(
        api,
        user,
        &file.data.secrets,
        &secrets_selections,
        params.secrets_passphrase.as_deref(),
        file.secrets_encryption.as_ref(),
    )
    .await;

    // 3. Import responders.
    let mut responders_result =
        import_responders(api, user, &file.data.responders, &responders_selections).await;

    // 4. Import certificate templates.
    let mut templates_result = import_certificate_templates(
        api,
        user,
        &file.data.certificate_templates,
        &templates_selections,
    )
    .await;

    // 5. Import private keys.
    let mut keys_result =
        import_private_keys(api, user, &file.data.private_keys, &keys_selections).await;

    // 6. Import CSPs.
    let mut csps_result = import_content_security_policies(
        api,
        user,
        &file.data.content_security_policies,
        &csps_selections,
    )
    .await;

    // 7. Import page trackers.
    let mut page_trackers_result = import_trackers(
        api,
        user,
        &file.data.page_trackers,
        &page_trackers_selections,
        TrackerKind::Page,
    )
    .await;

    // 8. Import API trackers.
    let mut api_trackers_result = import_trackers(
        api,
        user,
        &file.data.api_trackers,
        &api_trackers_selections,
        TrackerKind::Api,
    )
    .await;

    // 9. Apply mode deletions (reverse dependency order: trackers → CSPs → keys → templates → responders → secrets → scripts).
    if params.mode == ImportMode::Apply
        && let Some(ref deletions) = params.apply_deletions
    {
        // Delete page trackers.
        for id in &deletions.page_trackers {
            if api
                .web_scraping(user)
                .remove_page_tracker(*id)
                .await
                .is_ok()
            {
                page_trackers_result.deleted += 1;
            }
        }
        // Delete API trackers.
        for id in &deletions.api_trackers {
            if api.web_scraping(user).remove_api_tracker(*id).await.is_ok() {
                api_trackers_result.deleted += 1;
            }
        }
        // Delete CSPs.
        for id in &deletions.content_security_policies {
            if api
                .web_security()
                .remove_content_security_policy(user.id, *id)
                .await
                .is_ok()
            {
                csps_result.deleted += 1;
            }
        }
        // Delete private keys.
        for id in &deletions.private_keys {
            if api
                .certificates()
                .remove_private_key(user.id, *id)
                .await
                .is_ok()
            {
                keys_result.deleted += 1;
            }
        }
        // Delete certificate templates.
        for id in &deletions.certificate_templates {
            if api
                .certificates()
                .remove_certificate_template(user.id, *id)
                .await
                .is_ok()
            {
                templates_result.deleted += 1;
            }
        }
        // Delete responders.
        for id in &deletions.responders {
            if api.webhooks(user).remove_responder(*id).await.is_ok() {
                responders_result.deleted += 1;
            }
        }
        // Delete secrets.
        for id in &deletions.secrets {
            if api.secrets(user).delete_secret(*id).await.is_ok() {
                secrets_result.deleted += 1;
            }
        }
        // Delete scripts.
        for id in &deletions.scripts {
            if api.scripts(user).delete_script(*id).await.is_ok() {
                scripts_result.deleted += 1;
            }
        }
    }

    Ok(UserDataImportResult {
        results: ImportResultsSummary {
            scripts: scripts_result,
            secrets: secrets_result,
            responders: responders_result,
            certificate_templates: templates_result,
            private_keys: keys_result,
            content_security_policies: csps_result,
            page_trackers: page_trackers_result,
            api_trackers: api_trackers_result,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{mock_api_with_config, mock_config, mock_user};
    use file::{ImportedScript, UserDataImportFile, UserDataImportFileData};
    use params::{
        ImportAction, ImportEntitySelection, ImportMode, ImportSelections, UserDataImportParams,
        UserDataImportPreviewParams,
    };
    use sqlx::PgPool;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_empty_file() -> UserDataImportFile {
        UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts: vec![],
                secrets: vec![],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
            },
        }
    }

    #[sqlx::test]
    async fn preview_empty_file_merge(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let params = UserDataImportPreviewParams {
            data: make_empty_file(),
            mode: ImportMode::Merge,
        };

        let preview = generate_import_preview(&api, &user, &params).await?;
        assert!(preview.valid);
        assert_eq!(preview.version, 1);
        assert_eq!(preview.summary.scripts.total, 0);
        assert!(preview.to_delete.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn preview_rejects_unsupported_version(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let mut file = make_empty_file();
        file.version = 99;

        let params = UserDataImportPreviewParams {
            data: file,
            mode: ImportMode::Merge,
        };

        let preview = generate_import_preview(&api, &user, &params).await?;
        assert!(!preview.valid);
        assert!(!preview.warnings.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn preview_detects_script_conflict(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Create existing script.
        api.scripts(&user)
            .create_script("my_script", "responder", "content")
            .await?;

        let file = UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts: vec![ImportedScript {
                    id: Uuid::now_v7(),
                    name: "my_script".to_string(),
                    script_type: "responder".to_string(),
                    content: "new content".to_string(),
                    created_at: datetime!(2020-01-01 00:00:00 UTC),
                    updated_at: datetime!(2020-01-01 00:00:00 UTC),
                }],
                secrets: vec![],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
            },
        };

        let params = UserDataImportPreviewParams {
            data: file,
            mode: ImportMode::Merge,
        };

        let preview = generate_import_preview(&api, &user, &params).await?;
        assert!(preview.valid);
        assert_eq!(preview.summary.scripts.total, 1);
        assert_eq!(preview.summary.scripts.conflicts.len(), 1);
        assert_eq!(preview.summary.scripts.conflicts[0].name, "my_script");

        Ok(())
    }

    #[sqlx::test]
    async fn preview_apply_mode_detects_deletions(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Create an existing script that's NOT in the import file.
        api.scripts(&user)
            .create_script("orphan_script", "responder", "content")
            .await?;

        let params = UserDataImportPreviewParams {
            data: make_empty_file(),
            mode: ImportMode::Apply,
        };

        let preview = generate_import_preview(&api, &user, &params).await?;
        assert!(preview.valid);
        let to_delete = preview.to_delete.unwrap();
        assert_eq!(to_delete.scripts.len(), 1);
        assert_eq!(to_delete.scripts[0].name, "orphan_script");

        Ok(())
    }

    #[sqlx::test]
    async fn import_scripts_skip_action(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let script_id = Uuid::now_v7();
        let file = UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts: vec![ImportedScript {
                    id: script_id,
                    name: "skip_me".to_string(),
                    script_type: "responder".to_string(),
                    content: "content".to_string(),
                    created_at: datetime!(2020-01-01 00:00:00 UTC),
                    updated_at: datetime!(2020-01-01 00:00:00 UTC),
                }],
                secrets: vec![],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
            },
        };

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                scripts: vec![ImportEntitySelection {
                    source_id: script_id,
                    action: ImportAction::Skip,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.scripts.skipped, 1);
        assert_eq!(result.results.scripts.imported, 0);

        let scripts = api.scripts(&user).list_scripts(None).await?;
        assert!(scripts.is_empty());

        Ok(())
    }
}
