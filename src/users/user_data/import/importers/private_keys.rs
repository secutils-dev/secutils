use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::{
            export::ExportedPrivateKey,
            import::{
                ConflictResolution, ImportEntityResult, ImportEntitySelection, resolve_name,
                should_skip,
            },
        },
    },
    utils::certificates::PrivateKey,
};
use std::collections::{HashMap, HashSet};
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn import_private_keys<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    keys: &[ExportedPrivateKey],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing keys once for overwritten resolution.
    let existing_keys = api
        .certificates()
        .get_private_keys(user.id)
        .await
        .unwrap_or_default();
    let mut used_names: HashSet<_> = existing_keys.iter().map(|k| k.name.clone()).collect();

    for key in keys {
        let selection = selections.get(&key.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let name = resolve_name(&key.name, selection, &used_names);

        // Handle overwrite.
        if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite))
            && let Some(e) = existing_keys.iter().find(|k| k.name == key.name)
        {
            let _ = api.certificates().remove_private_key(user.id, e.id).await;
            used_names.remove(&key.name);
        }

        // Decode base64 PKCS#8.
        let pkcs8 = match openssl::base64::decode_block(&key.pkcs8) {
            Ok(bytes) => bytes,
            Err(err) => {
                result.failed += 1;
                result.errors.push(format!(
                    "Private key '{}': failed to decode PKCS#8: {err}",
                    key.name
                ));
                continue;
            }
        };

        let now = OffsetDateTime::now_utc();
        let private_key = PrivateKey {
            id: Uuid::now_v7(),
            name: name.clone(),
            alg: key.alg,
            pkcs8,
            encrypted: key.encrypted,
            created_at: now,
            updated_at: now,
        };

        match api
            .db
            .certificates()
            .insert_private_key(user.id, &private_key)
            .await
        {
            Ok(_) => {
                used_names.insert(name);
                result.imported += 1;
            }
            Err(err) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("Private key '{}': {err}", key.name));
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
        users::user_data::{export::ExportedPrivateKey, import::execute_import},
        utils::certificates::PrivateKeyAlgorithm,
    };
    use sqlx::PgPool;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_ed25519_pkcs8_base64() -> anyhow::Result<String> {
        let pkey = openssl::pkey::PKey::generate_ed25519()?;
        let pkcs8 = pkey.private_key_to_pkcs8()?;
        Ok(openssl::base64::encode_block(&pkcs8))
    }

    fn make_keys_file(keys: Vec<ExportedPrivateKey>) -> UserDataImportFile {
        UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts: vec![],
                secrets: vec![],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: keys,
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
                settings: None,
            },
        }
    }

    #[sqlx::test]
    async fn import_private_keys_merge(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let key_id = Uuid::now_v7();
        let pkcs8_b64 = make_ed25519_pkcs8_base64()?;
        let file = make_keys_file(vec![ExportedPrivateKey {
            id: key_id,
            name: "my_key".to_string(),
            alg: PrivateKeyAlgorithm::Ed25519,
            pkcs8: pkcs8_b64,
            encrypted: false,
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                private_keys: vec![ImportEntitySelection {
                    source_id: key_id,
                    action: ImportAction::Import,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.private_keys.imported, 1);
        assert_eq!(result.results.private_keys.failed, 0);

        let keys = api.certificates().get_private_keys(user.id).await?;
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "my_key");

        Ok(())
    }
}
