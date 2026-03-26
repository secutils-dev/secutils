use super::tags::remap_tag_ids;
use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::{
            export::ExportedTracker,
            import::{
                ConflictResolution, ImportEntityResult, ImportEntitySelection, resolve_name,
                should_skip,
            },
        },
    },
    utils::web_scraping::{
        ApiTrackerConfig, ApiTrackerCreateParams, ApiTrackerTarget, PageTrackerConfig,
        PageTrackerCreateParams, PageTrackerTarget,
    },
};
use retrack_types::trackers::{
    TrackerDataRevision, TrackerDataRevisionImportParams, TrackerTarget,
};
use std::collections::{HashMap, HashSet};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum TrackerKind {
    Page,
    Api,
}

pub async fn import_trackers<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    trackers: &[ExportedTracker],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
    kind: TrackerKind,
    tag_id_map: &HashMap<Uuid, Uuid>,
) -> ImportEntityResult {
    let web_scraping_api = api.web_scraping(user);
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing trackers once for overwrite resolution and name uniqueness.
    let existing_tracker_names: HashMap<String, Uuid> = match kind {
        TrackerKind::Page => web_scraping_api
            .get_page_trackers()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.name, t.id))
            .collect(),
        TrackerKind::Api => web_scraping_api
            .get_api_trackers()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.name, t.id))
            .collect(),
    };
    let mut used_names: HashSet<_> = existing_tracker_names.keys().cloned().collect();

    for tracker in trackers {
        let selection = selections.get(&tracker.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let name = resolve_name(&tracker.name, selection, &used_names);

        // Handle overwrite.
        if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite))
            && let Some(existing_id) = existing_tracker_names.get(&tracker.name)
        {
            match kind {
                TrackerKind::Page => {
                    let _ = web_scraping_api.remove_page_tracker(*existing_id).await;
                }
                TrackerKind::Api => {
                    let _ = web_scraping_api.remove_api_tracker(*existing_id).await;
                }
            }
            used_names.remove(&tracker.name);
        }

        let retrack = &tracker.retrack;

        match (&kind, &retrack.target) {
            (TrackerKind::Page, TrackerTarget::Page(page_target)) => {
                let create_params = PageTrackerCreateParams {
                    name: name.clone(),
                    enabled: retrack.enabled,
                    config: PageTrackerConfig {
                        revisions: retrack.config.revisions,
                        job: retrack.config.job.clone(),
                    },
                    target: PageTrackerTarget {
                        extractor: page_target.extractor.clone(),
                        accept_invalid_certificates: page_target.accept_invalid_certificates,
                        engine: page_target.engine,
                    },
                    notifications: retrack.notifications,
                    secrets: tracker.secrets.clone(),
                    tag_ids: remap_tag_ids(&tracker.tags, tag_id_map),
                };

                match web_scraping_api.create_page_tracker(create_params).await {
                    Ok(created) => {
                        used_names.insert(name);
                        result.imported += 1;
                        if !tracker.history.is_empty() {
                            import_tracker_history(api, created.retrack.id(), &tracker.history)
                                .await;
                        }
                    }
                    Err(err) => {
                        result.failed += 1;
                        result
                            .errors
                            .push(format!("Page tracker '{}': {err}", tracker.name));
                    }
                }
            }
            (TrackerKind::Api, TrackerTarget::Api(api_target)) => {
                // The Retrack ApiTarget stores requests as a Vec<TargetRequest>.
                // The secutils ApiTrackerTarget is a flattened single-request view.
                // Use the first request entry (export always produces exactly one).
                let request = match api_target.requests.first() {
                    Some(r) => r,
                    None => {
                        result.failed += 1;
                        result.errors.push(format!(
                            "API tracker '{}': no request defined in target",
                            tracker.name
                        ));
                        continue;
                    }
                };

                let url = request.url.clone();
                let method = request.method.as_ref().map(|m| m.to_string());
                let headers: Option<HashMap<String, String>> =
                    request.headers.as_ref().map(|header_map| {
                        header_map
                            .iter()
                            .map(|(k, v)| {
                                (k.to_string(), v.to_str().unwrap_or_default().to_string())
                            })
                            .collect()
                    });
                let body = request.body.clone();
                let media_type = request.media_type.as_ref().map(|m| m.to_string());
                let accept_statuses: Option<Vec<u16>> = request
                    .accept_statuses
                    .as_ref()
                    .map(|statuses| statuses.iter().map(|s| s.as_u16()).collect());
                let accept_invalid_certificates = request.accept_invalid_certificates;

                let create_params = ApiTrackerCreateParams {
                    name: name.clone(),
                    enabled: retrack.enabled,
                    config: ApiTrackerConfig {
                        revisions: retrack.config.revisions,
                        job: retrack.config.job.clone(),
                    },
                    target: ApiTrackerTarget {
                        url,
                        method,
                        headers,
                        body,
                        media_type,
                        accept_statuses,
                        accept_invalid_certificates,
                        configurator: api_target.configurator.clone(),
                        extractor: api_target.extractor.clone(),
                    },
                    notifications: retrack.notifications,
                    secrets: tracker.secrets.clone(),
                    tag_ids: remap_tag_ids(&tracker.tags, tag_id_map),
                };

                match web_scraping_api.create_api_tracker(create_params).await {
                    Ok(created) => {
                        used_names.insert(name);
                        result.imported += 1;
                        if !tracker.history.is_empty() {
                            import_tracker_history(api, created.retrack.id(), &tracker.history)
                                .await;
                        }
                    }
                    Err(err) => {
                        result.failed += 1;
                        result
                            .errors
                            .push(format!("API tracker '{}': {err}", tracker.name));
                    }
                }
            }
            _ => {
                result.failed += 1;
                result.errors.push(format!(
                    "Tracker '{}': target type does not match tracker kind",
                    tracker.name
                ));
            }
        }
    }
    result
}

async fn import_tracker_history<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    retrack_id: Uuid,
    history: &[TrackerDataRevision],
) {
    let import_params: Vec<TrackerDataRevisionImportParams> = history
        .iter()
        .map(|rev| TrackerDataRevisionImportParams {
            data: rev.data.clone(),
            created_at: rev.created_at,
        })
        .collect();

    if import_params.is_empty() {
        return;
    }

    if let Err(err) = api
        .retrack()
        .import_tracker_revisions(retrack_id, &import_params)
        .await
    {
        warn!(
            retrack.id = %retrack_id,
            "Failed to import tracker history ({} revisions): {err:?}",
            import_params.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::{
            UserDataImportParams,
            user_data::import::{
                ImportMode, execute_import,
                file::{UserDataImportFile, UserDataImportFileData},
                params::ImportSelections,
            },
        },
    };
    use sqlx::PgPool;
    use time::macros::datetime;

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
    async fn import_page_trackers_empty_handles_gracefully(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let params = UserDataImportParams {
            data: make_empty_file(),
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections::default(),
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.page_trackers.imported, 0);
        assert_eq!(result.results.page_trackers.failed, 0);

        Ok(())
    }
}
