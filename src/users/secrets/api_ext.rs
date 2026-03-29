use crate::{
    api::Api,
    error::Error,
    network::{DnsResolver, EmailTransport},
    users::{
        EntityTag, User,
        secrets::{SecretsAccess, SecretsEncryption, UserSecret},
    },
    utils::webhooks::{ResponderSettings, RespondersUpdateParams},
};
use serde::Deserialize;
use std::collections::HashMap;
use time::OffsetDateTime;
use tracing::error;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "GITHUB_TOKEN", "value": "ghp_xxxxxxxxxxxx", "tagIds": []}))]
pub struct SecretCreateParams {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
}

#[derive(Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"value": "ghp_yyyyyyyyyyyy"}))]
pub struct SecretUpdateParams {
    pub value: String,
    pub tag_ids: Option<Vec<Uuid>>,
}

/// Maximum length for a secret name.
const MAX_SECRET_NAME_LENGTH: usize = 128;
/// Maximum length for a secret value (10 KB).
const MAX_SECRET_VALUE_LENGTH: usize = 10 * 1024;

pub struct SecretsApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> SecretsApiExt<'a, 'u, DR, ET> {
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Lists all secrets for the user (metadata only, no values).
    pub async fn list_secrets(&self) -> anyhow::Result<Vec<UserSecret>> {
        let secrets_db = self.api.db.secrets();
        let mut secrets = secrets_db.get_user_secrets(self.user.id, false).await?;
        let ids: Vec<Uuid> = secrets.iter().map(|s| s.id).collect();
        let mut tags_map = secrets_db.get_secret_tags(&ids).await?;
        for secret in &mut secrets {
            secret.tags = tags_map.remove(&secret.id).unwrap_or_default();
        }
        Ok(secrets)
    }

    /// Returns secrets with the specified IDs.
    pub async fn bulk_get_secrets(&self, ids: &[Uuid]) -> anyhow::Result<Vec<UserSecret>> {
        let secrets_db = self.api.db.secrets();
        let mut secrets = secrets_db.bulk_get_user_secrets(self.user.id, ids).await?;
        let mut tags_map = secrets_db
            .get_secret_tags(&secrets.iter().map(|s| s.id).collect::<Vec<_>>())
            .await?;
        for secret in &mut secrets {
            secret.tags = tags_map.remove(&secret.id).unwrap_or_default();
        }
        Ok(secrets)
    }

    /// Creates a new secret after validating name, value, and subscription limits.
    pub async fn create_secret(&self, params: SecretCreateParams) -> anyhow::Result<UserSecret> {
        Self::validate_name(&params.name)?;
        Self::validate_value(&params.value)?;

        let secrets_db = self.api.db.secrets();
        let max_secrets = self
            .user
            .subscription
            .get_features(&self.api.config)
            .config
            .secrets
            .max_secrets;
        let count = secrets_db.count_user_secrets(self.user.id).await?;
        if count as usize >= max_secrets {
            return Err(anyhow::Error::from(Error::client(format!(
                "Maximum number of secrets ({max_secrets}) reached."
            ))));
        }

        let encryption = self.get_encryption()?;
        let encrypted_value = encryption.encrypt(params.value.as_bytes())?;

        // Preserve timestamp only up to seconds.
        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        let secret = UserSecret {
            id: Uuid::now_v7(),
            user_id: self.user.id,
            name: params.name,
            encrypted_value: Some(encrypted_value),
            tags: params.tag_ids.into_iter().map(EntityTag::from).collect(),
            created_at,
            updated_at: created_at,
        };

        let tags = secrets_db.insert_user_secret(self.user.id, &secret).await?;

        self.sync_tracker_secrets().await;
        Ok(UserSecret {
            tags,
            encrypted_value: None,
            ..secret
        })
    }

    /// Updates an existing secret's value.
    pub async fn update_secret(
        &self,
        id: Uuid,
        params: SecretUpdateParams,
    ) -> anyhow::Result<UserSecret> {
        Self::validate_value(&params.value)?;

        let secrets_db = self.api.db.secrets();
        let existing_secret = secrets_db
            .get_user_secrets(self.user.id, false)
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("Secret '{id}' not found.")))
            })?;

        let encryption = self.get_encryption()?;
        let encrypted_value = encryption.encrypt(params.value.as_bytes())?;

        let secret = UserSecret {
            encrypted_value: Some(encrypted_value),
            // Preserve timestamp only up to seconds.
            updated_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            ..existing_secret
        };

        let updated_tags = secrets_db
            .update_user_secret(self.user.id, &secret, params.tag_ids)
            .await?;

        self.sync_tracker_secrets().await;
        Ok(UserSecret {
            tags: updated_tags.unwrap_or(secret.tags),
            encrypted_value: None,
            ..secret
        })
    }

    /// Deletes a secret by id and cleans up dangling references in responders and trackers.
    pub async fn delete_secret(&self, id: Uuid) -> anyhow::Result<UserSecret> {
        let secret = self
            .api
            .db
            .secrets()
            .remove_user_secret(self.user.id, id)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("Secret '{id}' not found.")))
            })?;

        self.cleanup_deleted_secret(&secret.name).await;
        self.sync_tracker_secrets().await;
        Ok(secret)
    }

    /// Syncs secrets to any Retrack trackers that use them.
    async fn sync_tracker_secrets(&self) {
        let ws = self.api.web_scraping(self.user);
        if let Err(err) = ws.sync_secrets_to_trackers().await {
            error!(
                user.id = %self.user.id,
                "Failed to sync secrets to trackers: {err:?}"
            );
        }
    }

    /// Removes a deleted secret name from `Selected` lists in responders and trackers.
    async fn cleanup_deleted_secret(&self, name: &str) {
        // Clean up responder settings.
        if let Err(err) = self.cleanup_responder_secrets(name).await {
            error!(
                user.id = %self.user.id,
                secret.name = %name,
                "Failed to clean up responder secret references: {err:?}"
            );
        }

        // Clean up page tracker secrets (local DB column).
        if let Err(err) = self.cleanup_tracker_secrets(name).await {
            error!(
                user.id = %self.user.id,
                secret.name = %name,
                "Failed to clean up tracker secret references: {err:?}"
            );
        }
    }

    async fn cleanup_responder_secrets(&self, name: &str) -> anyhow::Result<()> {
        let webhooks_db = self.api.db.webhooks();
        let webhooks = self.api.webhooks(self.user);
        let responders = webhooks_db.get_responders(self.user.id).await?;
        for responder in responders {
            if let SecretsAccess::Selected { ref secrets } = responder.settings.secrets
                && secrets.contains(&name.to_string())
            {
                let _ = webhooks
                    .update_responder(
                        responder.id,
                        RespondersUpdateParams::with_settings(ResponderSettings {
                            secrets: responder.settings.secrets.without_secret(name),
                            ..responder.settings
                        }),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn cleanup_tracker_secrets(&self, name: &str) -> anyhow::Result<()> {
        let web_scraping_db = self.api.db.web_scraping(self.user.id);

        // Clean up page tracker secrets (local DB column only, no Retrack round-trip).
        let trackers = web_scraping_db.get_page_trackers().await?;
        for tracker in trackers {
            if let SecretsAccess::Selected { ref secrets } = tracker.secrets
                && secrets.contains(&name.to_string())
            {
                let updated = crate::utils::web_scraping::PageTracker {
                    secrets: tracker.secrets.without_secret(name),
                    ..tracker
                };
                web_scraping_db.update_page_tracker(&updated, None).await?;
            }
        }

        // Clean up API tracker secrets (local DB column only, no Retrack round-trip).
        let api_trackers = web_scraping_db.get_api_trackers().await?;
        for tracker in api_trackers {
            if let SecretsAccess::Selected { ref secrets } = tracker.secrets
                && secrets.contains(&name.to_string())
            {
                let updated = crate::utils::web_scraping::ApiTracker {
                    secrets: tracker.secrets.without_secret(name),
                    ..tracker
                };
                web_scraping_db.update_api_tracker(&updated, None).await?;
            }
        }

        Ok(())
    }

    /// Fetches and decrypts secrets according to the given access mode.
    /// Returns a map from a secret name to a decrypted string value.
    pub async fn get_decrypted_secrets(
        &self,
        access: &SecretsAccess,
    ) -> anyhow::Result<HashMap<String, String>> {
        match access {
            SecretsAccess::None => Ok(HashMap::new()),
            SecretsAccess::All => self.decrypt_all_secrets().await,
            SecretsAccess::Selected { secrets: names } => {
                if names.is_empty() {
                    return Ok(HashMap::new());
                }
                let all = self.decrypt_all_secrets().await?;
                Ok(all.into_iter().filter(|(k, _)| names.contains(k)).collect())
            }
        }
    }

    pub async fn decrypt_all_secrets(&self) -> anyhow::Result<HashMap<String, String>> {
        let encryption = self.get_encryption()?;
        let secrets = self
            .api
            .db
            .secrets()
            .get_user_secrets(self.user.id, true)
            .await?;
        let mut map = HashMap::with_capacity(secrets.len());
        for secret in secrets {
            if let Some(encrypted) = secret.encrypted_value {
                match encryption.decrypt(&encrypted) {
                    Ok(plaintext) => {
                        map.insert(
                            secret.name,
                            String::from_utf8_lossy(&plaintext).into_owned(),
                        );
                    }
                    Err(err) => {
                        error!(
                            user.id = %self.user.id,
                            secret.name = %secret.name,
                            "Failed to decrypt secret: {err:?}"
                        );
                    }
                }
            }
        }
        Ok(map)
    }

    fn get_encryption(&self) -> anyhow::Result<SecretsEncryption> {
        let key = self
            .api
            .config
            .security
            .secrets_encryption_key
            .as_deref()
            .ok_or_else(|| {
                anyhow::Error::from(Error::client("Secrets encryption key is not configured."))
            })?;
        SecretsEncryption::new(key)
    }

    fn validate_name(name: &str) -> anyhow::Result<()> {
        if !is_valid_secret_name(name) {
            return Err(anyhow::Error::from(Error::client(format!(
                "Secret name must start with a letter, contain only alphanumeric characters, \
                 underscores, or hyphens, and be at most {MAX_SECRET_NAME_LENGTH} characters."
            ))));
        }
        Ok(())
    }

    fn validate_value(value: &str) -> anyhow::Result<()> {
        if value.is_empty() {
            return Err(anyhow::Error::from(Error::client(
                "Secret value cannot be empty.",
            )));
        }
        if value.len() > MAX_SECRET_VALUE_LENGTH {
            return Err(anyhow::Error::from(Error::client(format!(
                "Secret value must be at most {} bytes.",
                MAX_SECRET_VALUE_LENGTH
            ))));
        }
        Ok(())
    }
}

fn is_valid_secret_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_SECRET_NAME_LENGTH {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with user secrets.
    pub fn secrets<'a, 'u>(&'a self, user: &'u User) -> SecretsApiExt<'a, 'u, DR, ET> {
        SecretsApiExt::new(self, user)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SECRET_VALUE_LENGTH, SecretCreateParams, SecretUpdateParams, is_valid_secret_name,
    };
    use crate::{
        retrack::tests::mock_retrack_tracker,
        tests::{mock_api_with_config, mock_config, mock_user},
        users::SecretsAccess,
        utils::webhooks::{
            Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
        },
    };
    use httpmock::MockServer;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use url::Url;
    use utoipa::PartialSchema;

    fn schema_example<T: PartialSchema>() -> serde_json::Value {
        match T::schema() {
            utoipa::openapi::RefOr::T(schema) => match schema {
                utoipa::openapi::Schema::Object(obj) => obj.example.expect("schema has no example"),
                _ => panic!("expected Object schema"),
            },
            utoipa::openapi::RefOr::Ref(_) => panic!("expected inline schema, got Ref"),
        }
    }

    #[test]
    fn secret_create_params_example_is_valid() {
        let example: SecretCreateParams =
            serde_json::from_value(schema_example::<SecretCreateParams>()).unwrap();
        assert!(is_valid_secret_name(&example.name));
        assert!(!example.value.is_empty());
        assert!(example.value.len() <= MAX_SECRET_VALUE_LENGTH);
    }

    #[test]
    fn secret_update_params_example_is_valid() {
        let example: SecretUpdateParams =
            serde_json::from_value(schema_example::<SecretUpdateParams>()).unwrap();
        assert!(!example.value.is_empty());
        assert!(example.value.len() <= MAX_SECRET_VALUE_LENGTH);
    }
    use uuid::uuid;

    const TEST_ENCRYPTION_KEY: &str =
        "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2";

    #[test]
    fn validates_secret_names() {
        assert!(is_valid_secret_name("API_KEY"));
        assert!(is_valid_secret_name("a"));
        assert!(is_valid_secret_name("My-Secret-123"));
        assert!(is_valid_secret_name("x_y_z"));
        assert!(is_valid_secret_name("A"));

        assert!(!is_valid_secret_name(""));
        assert!(!is_valid_secret_name("_starts_underscore"));
        assert!(!is_valid_secret_name("-starts-hyphen"));
        assert!(!is_valid_secret_name("123abc"));
        assert!(!is_valid_secret_name("has space"));
        assert!(!is_valid_secret_name("has.dot"));
        assert!(!is_valid_secret_name(&"a".repeat(129)));
        assert!(is_valid_secret_name(&"a".repeat(128)));
    }

    #[sqlx::test]
    async fn list_secrets_returns_empty_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets = api.secrets(&mock_user).list_secrets().await?;
        assert!(secrets.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn create_secret_validates_name(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "".into(),
                value: "value".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Secret name must"));

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "_invalid".into(),
                value: "value".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Secret name must"));

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "123abc".into(),
                value: "value".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Secret name must"));

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "a".repeat(129),
                value: "value".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Secret name must"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_secret_validates_value(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "VALID_NAME".into(),
                value: "".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Secret value cannot be empty"));

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "VALID_NAME".into(),
                value: "x".repeat(10 * 1024 + 1),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Secret value must be at most"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_secret_enforces_limit(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        config.subscriptions.ultimate.secrets.max_secrets = 2;
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        secrets_api
            .create_secret(SecretCreateParams {
                name: "KEY_A".into(),
                value: "val-a".into(),
                tag_ids: vec![],
            })
            .await?;
        secrets_api
            .create_secret(SecretCreateParams {
                name: "KEY_B".into(),
                value: "val-b".into(),
                tag_ids: vec![],
            })
            .await?;

        let err = secrets_api
            .create_secret(SecretCreateParams {
                name: "KEY_C".into(),
                value: "val-c".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Maximum number of secrets (2)"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_and_list_secrets(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let created = secrets_api
            .create_secret(SecretCreateParams {
                name: "MY_TOKEN".into(),
                value: "secret-val".into(),
                tag_ids: vec![],
            })
            .await?;
        assert_eq!(created.name, "MY_TOKEN");
        assert!(created.encrypted_value.is_none());

        let list = secrets_api.list_secrets().await?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "MY_TOKEN");
        assert!(list[0].encrypted_value.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_changes_value(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let created = secrets_api
            .create_secret(SecretCreateParams {
                name: "MY_KEY".into(),
                value: "old-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let updated = secrets_api
            .update_secret(
                created.id,
                SecretUpdateParams {
                    value: "new-value".into(),
                    tag_ids: None,
                },
            )
            .await?;
        assert_eq!(updated.name, "MY_KEY");

        let decrypted = secrets_api
            .get_decrypted_secrets(&SecretsAccess::All)
            .await?;
        assert_eq!(decrypted.get("MY_KEY").unwrap(), "new-value");

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_not_found(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .secrets(&mock_user)
            .update_secret(
                uuid::Uuid::now_v7(),
                SecretUpdateParams {
                    value: "val".into(),
                    tag_ids: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_removes_it(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let created = secrets_api
            .create_secret(SecretCreateParams {
                name: "TO_DELETE".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await?;
        assert_eq!(secrets_api.list_secrets().await?.len(), 1);

        let deleted = secrets_api.delete_secret(created.id).await?;
        assert_eq!(deleted.name, "TO_DELETE");
        assert!(secrets_api.list_secrets().await?.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_not_found(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .secrets(&mock_user)
            .delete_secret(uuid::Uuid::now_v7())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_cleans_up_responder_selected_list(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let secret_a = secrets_api
            .create_secret(SecretCreateParams {
                name: "KEY_A".into(),
                value: "val-a".into(),
                tag_ids: vec![],
            })
            .await?;
        secrets_api
            .create_secret(SecretCreateParams {
                name: "KEY_B".into(),
                value: "val-b".into(),
                tag_ids: vec![],
            })
            .await?;

        let now = OffsetDateTime::from_unix_timestamp(946720800)?;
        let responder = Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "resp".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/test".to_string(),
                subdomain_prefix: None,
            },
            method: ResponderMethod::Any,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 0,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::Selected {
                    secrets: vec!["KEY_A".to_string(), "KEY_B".to_string()],
                },
            },
            tags: vec![],
            created_at: now,
            updated_at: now,
        };
        api.db
            .webhooks()
            .insert_responder(mock_user.id, &responder)
            .await?;

        secrets_api.delete_secret(secret_a.id).await?;

        let responders = api.db.webhooks().get_responders(mock_user.id).await?;
        assert_eq!(responders.len(), 1);
        assert_eq!(
            responders[0].settings.secrets,
            SecretsAccess::Selected {
                secrets: vec!["KEY_B".to_string()]
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_collapses_responder_to_none_when_list_empty(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let only_key = secrets_api
            .create_secret(SecretCreateParams {
                name: "ONLY_KEY".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await?;

        let now = OffsetDateTime::from_unix_timestamp(946720800)?;
        let responder = Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "resp".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/test".to_string(),
                subdomain_prefix: None,
            },
            method: ResponderMethod::Any,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 0,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::Selected {
                    secrets: vec!["ONLY_KEY".to_string()],
                },
            },
            tags: vec![],
            created_at: now,
            updated_at: now,
        };
        api.db
            .webhooks()
            .insert_responder(mock_user.id, &responder)
            .await?;

        secrets_api.delete_secret(only_key.id).await?;

        let responders = api.db.webhooks().get_responders(mock_user.id).await?;
        assert_eq!(responders[0].settings.secrets, SecretsAccess::None);

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_does_not_touch_responder_with_all_access(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let key_x = secrets_api
            .create_secret(SecretCreateParams {
                name: "KEY_X".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await?;

        let now = OffsetDateTime::from_unix_timestamp(946720800)?;
        let responder = Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "resp".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/test".to_string(),
                subdomain_prefix: None,
            },
            method: ResponderMethod::Any,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 0,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::All,
            },
            tags: vec![],
            created_at: now,
            updated_at: now,
        };
        api.db
            .webhooks()
            .insert_responder(mock_user.id, &responder)
            .await?;

        secrets_api.delete_secret(key_x.id).await?;

        let responders = api.db.webhooks().get_responders(mock_user.id).await?;
        assert_eq!(responders[0].settings.secrets, SecretsAccess::All);

        Ok(())
    }

    fn mock_page_tracker(secrets: SecretsAccess) -> crate::utils::web_scraping::PageTracker {
        use crate::{retrack::RetrackTracker, utils::web_scraping::PageTracker};
        let now = OffsetDateTime::from_unix_timestamp(946720800).unwrap();
        PageTracker {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "tracker".to_string(),
            user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            retrack: RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            secrets,
            tags: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    fn mock_api_tracker_entity(secrets: SecretsAccess) -> crate::utils::web_scraping::ApiTracker {
        use crate::{retrack::RetrackTracker, utils::web_scraping::ApiTracker};
        let now = OffsetDateTime::from_unix_timestamp(946720800).unwrap();
        ApiTracker {
            id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "api-tracker".to_string(),
            user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            retrack: RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            secrets,
            tags: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    fn mock_retrack_api_tracker() -> anyhow::Result<retrack_types::trackers::Tracker> {
        use retrack_types::{
            scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
            trackers::{ApiTarget, TargetRequest, Tracker, TrackerConfig, TrackerTarget},
        };
        use std::time::Duration;

        Ok(Tracker {
            id: uuid!("00000000-0000-0000-0000-000000000020"),
            name: "api-tracker".to_string(),
            enabled: true,
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest::new(Url::parse(
                    "https://api.example.com/data",
                )?)],
                configurator: None,
                extractor: None,
                params: None,
            }),
            job_id: None,
            config: TrackerConfig {
                revisions: 3,
                timeout: None,
                job: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                }),
            },
            tags: vec![],
            actions: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            scheduled_at: None,
            last_ran_at: None,
        })
    }

    #[sqlx::test]
    async fn delete_secret_cleans_up_tracker_selected_list(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let tk_a = secrets_api
            .create_secret(SecretCreateParams {
                name: "TK_A".into(),
                value: "val-a".into(),
                tag_ids: vec![],
            })
            .await?;
        secrets_api
            .create_secret(SecretCreateParams {
                name: "TK_B".into(),
                value: "val-b".into(),
                tag_ids: vec![],
            })
            .await?;

        let tracker = mock_page_tracker(SecretsAccess::Selected {
            secrets: vec!["TK_A".to_string(), "TK_B".to_string()],
        });
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        secrets_api.delete_secret(tk_a.id).await?;

        let trackers = api
            .db
            .web_scraping(mock_user.id)
            .get_page_trackers()
            .await?;
        assert_eq!(trackers.len(), 1);
        assert_eq!(
            trackers[0].secrets,
            SecretsAccess::Selected {
                secrets: vec!["TK_B".to_string()]
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_collapses_tracker_to_none_when_list_empty(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let lone_key = secrets_api
            .create_secret(SecretCreateParams {
                name: "LONE_KEY".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await?;

        let tracker = mock_page_tracker(SecretsAccess::Selected {
            secrets: vec!["LONE_KEY".to_string()],
        });
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        secrets_api.delete_secret(lone_key.id).await?;

        let trackers = api
            .db
            .web_scraping(mock_user.id)
            .get_page_trackers()
            .await?;
        assert_eq!(trackers[0].secrets, SecretsAccess::None);

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_does_not_touch_tracker_with_all_access(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        let tk_x = secrets_api
            .create_secret(SecretCreateParams {
                name: "TK_X".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await?;

        let tracker = mock_page_tracker(SecretsAccess::All);
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        secrets_api.delete_secret(tk_x.id).await?;

        let trackers = api
            .db
            .web_scraping(mock_user.id)
            .get_page_trackers()
            .await?;
        assert_eq!(trackers[0].secrets, SecretsAccess::All);

        Ok(())
    }

    #[sqlx::test]
    async fn get_decrypted_secrets_respects_access_modes(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let secrets_api = api.secrets(&mock_user);
        secrets_api
            .create_secret(SecretCreateParams {
                name: "SEC_A".into(),
                value: "val-a".into(),
                tag_ids: vec![],
            })
            .await?;
        secrets_api
            .create_secret(SecretCreateParams {
                name: "SEC_B".into(),
                value: "val-b".into(),
                tag_ids: vec![],
            })
            .await?;
        secrets_api
            .create_secret(SecretCreateParams {
                name: "SEC_C".into(),
                value: "val-c".into(),
                tag_ids: vec![],
            })
            .await?;

        // None mode returns empty.
        let result = secrets_api
            .get_decrypted_secrets(&SecretsAccess::None)
            .await?;
        assert!(result.is_empty());

        // All mode returns all secrets.
        let result = secrets_api
            .get_decrypted_secrets(&SecretsAccess::All)
            .await?;
        assert_eq!(result.len(), 3);
        assert_eq!(result["SEC_A"], "val-a");
        assert_eq!(result["SEC_B"], "val-b");
        assert_eq!(result["SEC_C"], "val-c");

        // Selected mode returns only requested secrets.
        let result = secrets_api
            .get_decrypted_secrets(&SecretsAccess::Selected {
                secrets: vec!["SEC_A".to_string(), "SEC_C".to_string()],
            })
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result["SEC_A"], "val-a");
        assert_eq!(result["SEC_C"], "val-c");
        assert!(!result.contains_key("SEC_B"));

        // Selected mode with empty list returns empty.
        let result = secrets_api
            .get_decrypted_secrets(&SecretsAccess::Selected { secrets: vec![] })
            .await?;
        assert!(result.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn create_secret_fails_without_encryption_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "KEY".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Secrets encryption key is not configured")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn create_secret_syncs_to_tracker_with_all_secrets(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tracker = mock_page_tracker(SecretsAccess::All);
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_get_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_update_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .body_includes(r#""secrets":{"MY_KEY":"my-value"}"#);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        api.secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "MY_KEY".into(),
                value: "my-value".into(),
                tag_ids: vec![],
            })
            .await?;

        retrack_get_mock.assert();
        retrack_update_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_syncs_to_tracker_with_all_secrets(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        // Create secret first, before inserting the tracker so the initial sync is a no-op.
        let all_key = api
            .secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "ALL_KEY".into(),
                value: "old-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let tracker = mock_page_tracker(SecretsAccess::All);
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_get_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_update_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .body_includes(r#""secrets":{"ALL_KEY":"new-value"}"#);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        api.secrets(&mock_user)
            .update_secret(
                all_key.id,
                SecretUpdateParams {
                    value: "new-value".into(),
                    tag_ids: None,
                },
            )
            .await?;

        retrack_get_mock.assert();
        retrack_update_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_syncs_to_tracker_with_selected_secrets(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        // Create two secrets first. The tracker only uses SEL_KEY.
        let sel_key = api
            .secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "SEL_KEY".into(),
                value: "old-value".into(),
                tag_ids: vec![],
            })
            .await?;
        api.secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "OTHER".into(),
                value: "other-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let tracker = mock_page_tracker(SecretsAccess::Selected {
            secrets: vec!["SEL_KEY".to_string()],
        });
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_get_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_update_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .body_includes(r#""secrets":{"SEL_KEY":"new-value"}"#);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        api.secrets(&mock_user)
            .update_secret(
                sel_key.id,
                SecretUpdateParams {
                    value: "new-value".into(),
                    tag_ids: None,
                },
            )
            .await?;

        retrack_get_mock.assert();
        retrack_update_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_syncs_to_api_tracker_with_all_secrets(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        // Create secret first, before inserting the tracker so the initial sync is a no-op.
        let all_key = api
            .secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "ALL_KEY".into(),
                value: "old-value".into(),
                tag_ids: vec![],
            })
            .await?;

        // Only an API tracker (no page trackers) – this specifically tests that
        // sync_secrets_to_trackers does not skip API trackers when the page tracker
        // list is empty.
        let tracker = mock_api_tracker_entity(SecretsAccess::All);
        api.db
            .web_scraping(mock_user.id)
            .insert_api_tracker(&tracker)
            .await?;

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_get_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_update_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .body_includes(r#""secrets":{"ALL_KEY":"new-value"}"#);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        api.secrets(&mock_user)
            .update_secret(
                all_key.id,
                SecretUpdateParams {
                    value: "new-value".into(),
                    tag_ids: None,
                },
            )
            .await?;

        retrack_get_mock.assert();
        retrack_update_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn delete_secret_syncs_to_tracker(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        api.secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "KEEP".into(),
                value: "keep-val".into(),
                tag_ids: vec![],
            })
            .await?;
        let del_secret = api
            .secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "DEL".into(),
                value: "del-val".into(),
                tag_ids: vec![],
            })
            .await?;

        let tracker = mock_page_tracker(SecretsAccess::All);
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_get_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        // After deleting DEL, only KEEP remains.
        let retrack_update_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .body_includes(r#""secrets":{"KEEP":"keep-val"}"#);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        api.secrets(&mock_user).delete_secret(del_secret.id).await?;

        retrack_get_mock.assert();
        retrack_update_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn create_secret_skips_sync_when_no_trackers_use_secrets(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key = Some(TEST_ENCRYPTION_KEY.to_string());

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        // Tracker with SecretsAccess::None - sync should be skipped entirely.
        let tracker = mock_page_tracker(SecretsAccess::None);
        api.db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let retrack_get_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path_includes("/api/trackers/");
            then.status(200);
        });

        api.secrets(&mock_user)
            .create_secret(SecretCreateParams {
                name: "IGNORED".into(),
                value: "val".into(),
                tag_ids: vec![],
            })
            .await?;

        // Retrack should never have been called.
        retrack_get_mock.assert_calls(0);

        Ok(())
    }
}
