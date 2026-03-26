use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        SecretCreateParams, User,
        secrets::{SecretsEncryptionMeta, decrypt_secret_from_export},
        user_data::{
            import::{
                ConflictResolution, ImportEntityResult, ImportEntitySelection, remap_tag_ids,
                resolve_name, should_skip,
            },
            shared::DataFileSecret,
        },
    },
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub async fn import_secrets<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    secrets: &[DataFileSecret],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
    passphrase: Option<&str>,
    encryption_meta: Option<&SecretsEncryptionMeta>,
    tag_id_map: &HashMap<Uuid, Uuid>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing secrets once for conflict detection.
    let secrets_api = api.secrets(user);
    let existing_secrets = secrets_api.list_secrets().await.unwrap_or_default();
    let mut used_names: HashSet<String> = existing_secrets.iter().map(|s| s.name.clone()).collect();

    for secret in secrets {
        let selection = selections.get(&secret.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        // Decrypt the value from the export file (or use placeholder).
        let value = match decrypt_import_secret_value(secret, passphrase, encryption_meta) {
            Ok(v) => v,
            Err(err) => {
                result.failed += 1;
                result.errors.push(err);
                continue;
            }
        };

        // Check for name conflict.
        if used_names.contains(&secret.name) {
            let resolution = selection.and_then(|s| s.conflict_resolution);
            match resolution {
                Some(ConflictResolution::Rename) => {
                    let new_name = resolve_name(&secret.name, selection, &used_names);
                    let remapped_tags = remap_tag_ids(&secret.tags, tag_id_map);
                    match secrets_api
                        .create_secret(SecretCreateParams {
                            name: new_name.clone(),
                            value: value.clone(),
                            tag_ids: remapped_tags,
                        })
                        .await
                    {
                        Ok(_) => {
                            used_names.insert(new_name);
                            result.imported += 1;
                        }
                        Err(err) => {
                            result.failed += 1;
                            result
                                .errors
                                .push(format!("Secret '{}': {}", secret.name, err));
                        }
                    }
                }
                Some(ConflictResolution::Overwrite) => {
                    // Delete existing and create new with the imported value.
                    if let Some(e) = existing_secrets.iter().find(|s| s.name == secret.name) {
                        if secret.encrypted_value.is_some() {
                            // Only overwrite if we have a real value to import.
                            let _ = secrets_api.delete_secret(e.id).await;
                            used_names.remove(&secret.name);
                            let remapped_tags = remap_tag_ids(&secret.tags, tag_id_map);
                            match secrets_api
                                .create_secret(SecretCreateParams {
                                    name: secret.name.clone(),
                                    value: value.clone(),
                                    tag_ids: remapped_tags,
                                })
                                .await
                            {
                                Ok(_) => {
                                    used_names.insert(secret.name.clone());
                                    result.updated += 1;
                                }
                                Err(err) => {
                                    result.failed += 1;
                                    result
                                        .errors
                                        .push(format!("Secret '{}': {err}", secret.name));
                                }
                            }
                        } else {
                            // No value to overwrite with, skip.
                            result.skipped += 1;
                        }
                    } else {
                        result.skipped += 1;
                    }
                }
                Some(ConflictResolution::Skip) | None => {
                    result.skipped += 1;
                }
            }
            continue;
        }

        let remapped_tags = remap_tag_ids(&secret.tags, tag_id_map);
        match secrets_api
            .create_secret(SecretCreateParams {
                name: secret.name.clone(),
                value,
                tag_ids: remapped_tags,
            })
            .await
        {
            Ok(_) => {
                used_names.insert(secret.name.clone());
                result.imported += 1;
            }
            Err(err) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("Secret '{}': {err}", secret.name));
            }
        }
    }
    result
}

/// Decrypts a secret value from the import file if passphrase and encryption metadata are available.
/// Returns the plaintext string, or "placeholder" if no encrypted value is present.
fn decrypt_import_secret_value(
    secret: &DataFileSecret,
    passphrase: Option<&str>,
    encryption_meta: Option<&SecretsEncryptionMeta>,
) -> Result<String, String> {
    match (&secret.encrypted_value, passphrase, encryption_meta) {
        (Some(encrypted), Some(pass), Some(meta)) => {
            let bytes = decrypt_secret_from_export(encrypted, pass, meta)
                .map_err(|e| format!("Failed to decrypt secret '{}': {e}", secret.name))?;
            String::from_utf8(bytes)
                .map_err(|e| format!("Secret '{}' decrypted to invalid UTF-8: {e}", secret.name))
        }
        (Some(_), _, _) => Err(format!(
            "Secret '{}' has an encrypted value but no passphrase was provided.",
            secret.name
        )),
        _ => Ok("placeholder".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::user_data::import::{
            execute_import,
            file::{UserDataImportFile, UserDataImportFileData},
            params::{
                ImportAction, ImportEntitySelection, ImportMode, ImportSelections,
                UserDataImportParams,
            },
        },
    };
    use sqlx::PgPool;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_secret(id: Uuid, name: &str, encrypted_value: Option<String>) -> DataFileSecret {
        DataFileSecret {
            id,
            name: name.to_string(),
            encrypted_value,
            tags: vec![],
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }
    }

    fn make_secrets_file(secrets: Vec<DataFileSecret>) -> UserDataImportFile {
        UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                tags: vec![],
                scripts: vec![],
                secrets,
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

    #[test]
    fn decrypt_import_secret_value_returns_placeholder_when_no_encrypted_value() {
        let secret = make_secret(Uuid::nil(), "my_secret", None);
        let result = decrypt_import_secret_value(&secret, None, None);
        assert_eq!(result.unwrap(), "placeholder");
    }

    const TEST_ENCRYPTION_KEY: &str =
        "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2";

    #[sqlx::test]
    async fn import_secrets_merge(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let secret_id = Uuid::now_v7();
        let file = make_secrets_file(vec![make_secret(secret_id, "my_secret", None)]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                secrets: vec![ImportEntitySelection {
                    source_id: secret_id,
                    action: ImportAction::Import,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.secrets.imported, 1);
        assert_eq!(result.results.secrets.failed, 0);

        let secrets = api.secrets(&user).list_secrets().await?;
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].name, "my_secret");

        Ok(())
    }

    #[sqlx::test]
    async fn import_secrets_skips_on_skip_action(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let secret_id = Uuid::now_v7();
        let file = make_secrets_file(vec![make_secret(secret_id, "skip_me", None)]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                secrets: vec![ImportEntitySelection {
                    source_id: secret_id,
                    action: ImportAction::Skip,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.secrets.skipped, 1);
        assert_eq!(result.results.secrets.imported, 0);

        let secrets = api.secrets(&user).list_secrets().await?;
        assert!(secrets.is_empty());

        Ok(())
    }
}
