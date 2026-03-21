use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::import::{
            ConflictResolution, ImportEntityResult, ImportEntitySelection, resolve_name,
            should_skip,
        },
    },
    utils::certificates::CertificateTemplate,
};
use std::collections::{HashMap, HashSet};
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn import_certificate_templates<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    templates: &[CertificateTemplate],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing templates once for overwritten resolution.
    let existing_templates = api
        .certificates()
        .get_certificate_templates(user.id)
        .await
        .unwrap_or_default();
    let mut used_names: HashSet<_> = existing_templates.iter().map(|t| t.name.clone()).collect();

    for template in templates {
        let selection = selections.get(&template.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let resolved_name = resolve_name(&template.name, selection, &used_names);

        // Handle overwrite.
        if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite))
            && let Some(e) = existing_templates.iter().find(|t| t.name == template.name)
        {
            let _ = api
                .certificates()
                .remove_certificate_template(user.id, e.id)
                .await;
            used_names.remove(&template.name);
        }

        let mut new_template = template.clone();
        new_template.id = Uuid::now_v7();
        new_template.name = resolved_name.clone();
        let now = OffsetDateTime::now_utc();
        new_template.created_at = now;
        new_template.updated_at = now;

        match api
            .db
            .certificates()
            .insert_certificate_template(user.id, &new_template)
            .await
        {
            Ok(_) => {
                used_names.insert(resolved_name);
                result.imported += 1;
            }
            Err(err) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("Template '{}': {err}", template.name));
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
        utils::certificates::{
            CertificateAttributes, CertificateTemplate, PrivateKeyAlgorithm, SignatureAlgorithm,
            Version,
        },
    };
    use sqlx::PgPool;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_certificate_template(id: Uuid, name: &str) -> CertificateTemplate {
        CertificateTemplate {
            id,
            name: name.to_string(),
            attributes: CertificateAttributes {
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Ed25519,
                not_valid_before: datetime!(2020-01-01 00:00:00 UTC),
                not_valid_after: datetime!(2025-01-01 00:00:00 UTC),
                version: Version::Three,
                is_ca: false,
                common_name: None,
                country: None,
                state_or_province: None,
                locality: None,
                organization: None,
                organizational_unit: None,
                key_usage: None,
                extended_key_usage: None,
            },
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }
    }

    fn make_ct_file(templates: Vec<CertificateTemplate>) -> UserDataImportFile {
        UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts: vec![],
                secrets: vec![],
                responders: vec![],
                certificate_templates: templates,
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
            },
        }
    }

    #[sqlx::test]
    async fn import_certificate_templates_merge(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let template_id = Uuid::now_v7();
        let file = make_ct_file(vec![make_certificate_template(template_id, "my_template")]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                certificate_templates: vec![ImportEntitySelection {
                    source_id: template_id,
                    action: ImportAction::Import,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.certificate_templates.imported, 1);
        assert_eq!(result.results.certificate_templates.failed, 0);

        let templates = api
            .certificates()
            .get_certificate_templates(user.id)
            .await?;
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "my_template");

        Ok(())
    }
}
