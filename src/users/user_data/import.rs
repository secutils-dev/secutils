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
    importers::tags::remap_tag_ids,
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
    utils::webhooks::Responder,
};
use detect_conflicts::{detect_conflicts, detect_responder_conflicts};
use detect_deletions::detect_deletions;
use detect_duplicates::detect_intra_file_duplicates;
use importers::{
    certificate_templates::import_certificate_templates,
    content_security_policies::import_content_security_policies,
    private_keys::import_private_keys,
    responders::import_responders,
    scripts::import_scripts,
    secrets::import_secrets,
    tags::import_tags,
    trackers::{TrackerKind, import_trackers},
};
use results::{
    ApplyDeleteItem, ApplyDeleteSummary, ImportEntitySummary, ImportPreviewSummary,
    ImportResultsSummary, ImportSettingsSummary, UserDataImportPreview, UserDataImportResult,
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
    let import_tags: Vec<(Uuid, &str)> = file
        .data
        .tags
        .iter()
        .map(|t| (t.id, t.name.as_str()))
        .collect();
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
    let existing_tags = fetch_existing(!import_tags.is_empty() || is_apply, || async {
        Ok(api
            .tags(user)
            .list_tags()
            .await?
            .into_iter()
            .map(|t| (t.id, t.name))
            .collect())
    })
    .await?;

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

    let existing_responders: Vec<Responder> = if !import_responders.is_empty() || is_apply {
        api.webhooks(user).get_responders().await?
    } else {
        Vec::new()
    };

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

    // Use database-level queries for trackers instead of the API-level methods,
    // since we only need (id, name) pairs for conflict/deletion detection and
    // don't need Retrack service enrichment.
    let web_scraping_db = api.db.web_scraping(user.id);
    let existing_page_trackers =
        fetch_existing(!import_page_trackers.is_empty() || is_apply, || async {
            Ok(web_scraping_db
                .get_page_trackers()
                .await?
                .into_iter()
                .map(|t| (t.id, t.name))
                .collect())
        })
        .await?;

    let existing_api_trackers =
        fetch_existing(!import_api_trackers.is_empty() || is_apply, || async {
            Ok(web_scraping_db
                .get_api_trackers()
                .await?
                .into_iter()
                .map(|t| (t.id, t.name))
                .collect())
        })
        .await?;

    // Compute summaries and deletion candidates for each entity type.
    let (tags_summary, tags_deletions) = entity_preview(&import_tags, &existing_tags, is_apply);
    let (scripts_summary, scripts_deletions) =
        entity_preview(&import_scripts, &existing_scripts, is_apply);
    let (secrets_summary, _) = entity_preview(&import_secrets, &existing_secrets, is_apply);
    // Responders use a dedicated conflict detector that checks location+method in addition to name.
    let import_responder_refs: Vec<&Responder> =
        file.data.responders.iter().map(|r| &r.responder).collect();
    let existing_responder_refs: Vec<&Responder> = existing_responders.iter().collect();
    let responders_summary = ImportEntitySummary {
        total: import_responder_refs.len(),
        conflicts: detect_responder_conflicts(&import_responder_refs, &existing_responder_refs),
    };
    let existing_responder_pairs: Vec<(Uuid, String)> = existing_responders
        .iter()
        .map(|r| (r.id, r.name.clone()))
        .collect();
    let responders_deletions = if is_apply {
        let existing_refs: Vec<(Uuid, &str)> = existing_responder_pairs
            .iter()
            .map(|(id, name)| (*id, name.as_str()))
            .collect();
        let import_names: Vec<&str> = import_responders.iter().map(|(_, n)| *n).collect();
        detect_deletions(&import_names, &existing_refs)
    } else {
        Vec::new()
    };
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

    // Settings preview: check if the file includes settings and if the user has existing settings.
    let settings_included = file.data.settings.is_some();
    let has_existing_settings = if settings_included || is_apply {
        api.settings(user).get_settings().await?.is_some()
    } else {
        false
    };

    let to_delete = is_apply.then_some(ApplyDeleteSummary {
        tags: tags_deletions,
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
            tags: tags_summary,
            scripts: scripts_summary,
            secrets: secrets_summary,
            responders: responders_summary,
            certificate_templates: templates_summary,
            private_keys: keys_summary,
            content_security_policies: csps_summary,
            page_trackers: page_trackers_summary,
            api_trackers: api_trackers_summary,
            settings: ImportSettingsSummary {
                included: settings_included,
                has_existing: has_existing_settings,
            },
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
    let tags_selections = selection_map!(params.selections.tags);
    let scripts_selections = selection_map!(params.selections.scripts);
    let secrets_selections = selection_map!(params.selections.secrets);
    let responders_selections = selection_map!(params.selections.responders);
    let templates_selections = selection_map!(params.selections.certificate_templates);
    let keys_selections = selection_map!(params.selections.private_keys);
    let csps_selections = selection_map!(params.selections.content_security_policies);
    let page_trackers_selections = selection_map!(params.selections.page_trackers);
    let api_trackers_selections = selection_map!(params.selections.api_trackers);

    // 1. Import tags (must run before entities that reference tag IDs).
    // Filter to only selected tags; deselected tags won't enter the tag_id_map,
    // so remap_tag_ids() will naturally drop them from imported entities.
    let file_tags: Vec<_> = file
        .data
        .tags
        .iter()
        .filter(|t| !should_skip(tags_selections.get(&t.id)))
        .cloned()
        .collect();
    let skipped_tags = file.data.tags.len() - file_tags.len();
    let (tag_id_map, mut tags_result) = import_tags(api, user, &file_tags, &tags_selections).await;
    tags_result.skipped += skipped_tags;

    // 2. Import scripts.
    let mut scripts_result = import_scripts(
        api,
        user,
        &file.data.scripts,
        &scripts_selections,
        &tag_id_map,
    )
    .await;

    // 3. Import secrets.
    let mut secrets_result = import_secrets(
        api,
        user,
        &file.data.secrets,
        &secrets_selections,
        params.secrets_passphrase.as_deref(),
        file.secrets_encryption.as_ref(),
        &tag_id_map,
    )
    .await;

    // 4. Import responders.
    let mut responders_result = import_responders(
        api,
        user,
        &file.data.responders,
        &responders_selections,
        &tag_id_map,
    )
    .await;

    // 5. Import certificate templates.
    let mut templates_result = import_certificate_templates(
        api,
        user,
        &file.data.certificate_templates,
        &templates_selections,
        &tag_id_map,
    )
    .await;

    // 6. Import private keys.
    let mut keys_result = import_private_keys(
        api,
        user,
        &file.data.private_keys,
        &keys_selections,
        &tag_id_map,
    )
    .await;

    // 7. Import CSPs.
    let mut csps_result = import_content_security_policies(
        api,
        user,
        &file.data.content_security_policies,
        &csps_selections,
        &tag_id_map,
    )
    .await;

    // 8. Import page trackers.
    let mut page_trackers_result = import_trackers(
        api,
        user,
        &file.data.page_trackers,
        &page_trackers_selections,
        TrackerKind::Page,
        &tag_id_map,
    )
    .await;

    // 9. Import API trackers.
    let mut api_trackers_result = import_trackers(
        api,
        user,
        &file.data.api_trackers,
        &api_trackers_selections,
        TrackerKind::Api,
        &tag_id_map,
    )
    .await;

    // 10. Import settings.
    let mut settings_result = ImportEntityResult::default();
    if params.selections.import_settings {
        if let Some(ref settings) = file.data.settings {
            match api.settings(user).replace_settings(settings).await {
                Ok(_) => settings_result.imported = 1,
                Err(err) => {
                    settings_result.failed = 1;
                    settings_result
                        .errors
                        .push(format!("Failed to import settings: {err}"));
                }
            }
        }
    } else if file.data.settings.is_some() {
        settings_result.skipped = 1;
    }

    // 11. Apply mode deletions.
    // Tags are auto-deleted when not in the import file (no explicit user confirmation needed).
    if params.mode == ImportMode::Apply {
        let import_tag_names: std::collections::HashSet<&str> =
            file.data.tags.iter().map(|t| t.name.as_str()).collect();
        let tags = api.tags(user);
        if let Ok(existing_tags) = tags.list_tags().await {
            for existing_tag in existing_tags {
                if !import_tag_names.contains(existing_tag.name.as_str())
                    && tags.delete_tag(existing_tag.id).await.is_ok()
                {
                    tags_result.deleted += 1;
                }
            }
        }
    }

    // Entity deletions require explicit user confirmation via apply_deletions.
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
            tags: tags_result,
            scripts: scripts_result,
            secrets: secrets_result,
            responders: responders_result,
            certificate_templates: templates_result,
            private_keys: keys_result,
            content_security_policies: csps_result,
            page_trackers: page_trackers_result,
            api_trackers: api_trackers_result,
            settings: settings_result,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::scripts::ScriptCreateParams,
    };
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
                tags: vec![],
                scripts: vec![],
                secrets: vec![],
                responders: vec![],
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
            .create_script(ScriptCreateParams {
                name: "my_script".into(),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;

        let file = UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                tags: vec![],
                scripts: vec![ImportedScript {
                    id: Uuid::now_v7(),
                    name: "my_script".to_string(),
                    script_type: "responder".to_string(),
                    content: "new content".to_string(),
                    tags: vec![],
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
                settings: None,
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
            .create_script(ScriptCreateParams {
                name: "orphan_script".into(),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
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
                tags: vec![],
                scripts: vec![ImportedScript {
                    id: script_id,
                    name: "skip_me".to_string(),
                    script_type: "responder".to_string(),
                    content: "content".to_string(),
                    tags: vec![],
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
                settings: None,
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
