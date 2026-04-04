use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::import::{
            ConflictResolution, ImportEntityResult, ImportEntitySelection, remap_tag_ids,
            resolve_name, should_skip,
        },
    },
    utils::web_security::{
        ContentSecurityPoliciesCreateParams, ContentSecurityPolicy, ContentSecurityPolicyContent,
    },
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub async fn import_content_security_policies<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    csps: &[ContentSecurityPolicy],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
    tag_id_map: &HashMap<Uuid, Uuid>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing CSPs once for overwritten resolution.
    let web_security_api = api.web_security(user);
    let existing_csps = web_security_api
        .get_content_security_policies()
        .await
        .unwrap_or_default();
    let mut used_names: HashSet<_> = existing_csps.iter().map(|c| c.name.clone()).collect();

    for csp in csps {
        let selection = selections.get(&csp.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let resolved_name = resolve_name(&csp.name, selection, &used_names);

        // Handle overwrite.
        if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite))
            && let Some(e) = existing_csps.iter().find(|c| c.name == csp.name)
        {
            let _ = web_security_api.remove_content_security_policy(e.id).await;
            used_names.remove(&csp.name);
        }

        match web_security_api
            .create_content_security_policy(ContentSecurityPoliciesCreateParams {
                name: resolved_name.clone(),
                content: ContentSecurityPolicyContent::Directives(csp.directives.clone()),
                tag_ids: remap_tag_ids(&csp.tags, tag_id_map),
            })
            .await
        {
            Ok(_) => {
                used_names.insert(resolved_name);
                result.imported += 1;
            }
            Err(err) => {
                result.failed += 1;
                result.errors.push(format!("CSP '{}': {err}", csp.name));
            }
        }
    }
    result
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
        users::user_data::import::execute_import,
        utils::web_security::{ContentSecurityPolicy, ContentSecurityPolicyDirective},
    };
    use sqlx::PgPool;
    use std::collections::BTreeSet;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_csp(id: Uuid, name: &str) -> ContentSecurityPolicy {
        ContentSecurityPolicy {
            id,
            name: name.to_string(),
            directives: vec![ContentSecurityPolicyDirective::DefaultSrc(BTreeSet::from(
                ["'self'".to_string()],
            ))],
            tags: vec![],
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }
    }

    fn make_csp_file(csps: Vec<ContentSecurityPolicy>) -> UserDataImportFile {
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
                content_security_policies: csps,
                page_trackers: vec![],
                api_trackers: vec![],
                settings: None,
            },
        }
    }

    #[sqlx::test]
    async fn import_content_security_policies_merge(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let csp_id = Uuid::now_v7();
        let file = make_csp_file(vec![make_csp(csp_id, "my_csp")]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                content_security_policies: vec![ImportEntitySelection {
                    source_id: csp_id,
                    action: ImportAction::Import,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.content_security_policies.imported, 1);
        assert_eq!(result.results.content_security_policies.failed, 0);

        let csps = api
            .web_security(&user)
            .get_content_security_policies()
            .await?;
        assert_eq!(csps.len(), 1);
        assert_eq!(csps[0].name, "my_csp");

        Ok(())
    }
}
