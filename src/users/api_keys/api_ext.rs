use crate::{
    api::Api,
    error::Error,
    network::{DnsResolver, EmailTransport},
    users::{User, api_keys::UserApiKey},
};
use openssl::hash::MessageDigest;
use serde::Deserialize;
use serde_with::{TimestampSeconds, serde_as};
use time::OffsetDateTime;
use tracing::warn;
use utoipa::ToSchema;
use uuid::Uuid;

/// Token prefix used to distinguish API keys from JWTs in the Bearer header.
pub const API_KEY_TOKEN_PREFIX: &str = "su_ak_";

/// Maximum length for an API key name.
const MAX_API_KEY_NAME_LENGTH: usize = 128;

#[serde_as]
#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "CI deployment key", "expiresAt": 1750000000}))]
pub struct ApiKeyCreateParams {
    pub name: String,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "Production agent key"}))]
pub struct ApiKeyUpdateParams {
    pub name: String,
}

#[serde_as]
#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"expiresAt": 1760000000}))]
pub struct ApiKeyRegenerateParams {
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    pub expires_at: Option<OffsetDateTime>,
}

pub struct ApiKeysApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> ApiKeysApiExt<'a, 'u, DR, ET> {
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Lists all API keys for the user (including expired ones).
    pub async fn list_api_keys(&self) -> anyhow::Result<Vec<UserApiKey>> {
        self.api.db.api_keys().get_user_api_keys(self.user.id).await
    }

    /// Creates a new API key. Returns the key metadata and the plaintext token (shown once).
    pub async fn create_api_key(
        &self,
        params: ApiKeyCreateParams,
    ) -> anyhow::Result<(UserApiKey, String)> {
        Self::validate_name(&params.name)?;

        if let Some(expires_at) = params.expires_at {
            Self::validate_expires_at(expires_at)?;
        }

        let max_api_keys = self.api.config.security.max_user_api_keys;
        let api_keys_db = self.api.db.api_keys();
        let count = api_keys_db.count_user_api_keys(self.user.id).await?;
        if count as usize >= max_api_keys {
            return Err(anyhow::Error::from(Error::client(format!(
                "Maximum number of API keys ({max_api_keys}) reached."
            ))));
        }

        let (plaintext, token_hash) = Self::generate_token()?;
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        let api_key = UserApiKey {
            id: Uuid::now_v7(),
            user_id: self.user.id,
            name: params.name,
            token_hash,
            created_at: now,
            updated_at: now,
            expires_at: params.expires_at,
            last_used_at: None,
        };

        api_keys_db
            .insert_user_api_key(self.user.id, &api_key)
            .await?;

        Ok((api_key, plaintext))
    }

    /// Updates only the name of an existing API key.
    pub async fn update_api_key(
        &self,
        id: Uuid,
        params: ApiKeyUpdateParams,
    ) -> anyhow::Result<UserApiKey> {
        Self::validate_name(&params.name)?;

        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        self.api
            .db
            .api_keys()
            .update_user_api_key_name(self.user.id, id, &params.name, now)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("API key '{id}' not found.")))
            })
    }

    /// Deletes an API key by id.
    pub async fn delete_api_key(&self, id: Uuid) -> anyhow::Result<UserApiKey> {
        self.api
            .db
            .api_keys()
            .remove_user_api_key(self.user.id, id)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("API key '{id}' not found.")))
            })
    }

    /// Regenerates the token for an existing API key. The old token is invalidated
    /// immediately. Returns the updated key metadata and the new plaintext token.
    pub async fn regenerate_api_key(
        &self,
        id: Uuid,
        params: ApiKeyRegenerateParams,
    ) -> anyhow::Result<(UserApiKey, String)> {
        if let Some(expires_at) = params.expires_at {
            Self::validate_expires_at(expires_at)?;
        }

        let (plaintext, token_hash) = Self::generate_token()?;
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;

        let api_key = self
            .api
            .db
            .api_keys()
            .update_user_api_key_token(self.user.id, id, &token_hash, params.expires_at, now)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("API key '{id}' not found.")))
            })?;

        Ok((api_key, plaintext))
    }

    /// Generates a random token and its SHA-256 hash.
    fn generate_token() -> anyhow::Result<(String, Vec<u8>)> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)?;
        let plaintext = format!("{API_KEY_TOKEN_PREFIX}{}", hex::encode(bytes));
        let hash = openssl::hash::hash(MessageDigest::sha256(), plaintext.as_bytes())?.to_vec();
        Ok((plaintext, hash))
    }

    fn validate_name(name: &str) -> anyhow::Result<()> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_API_KEY_NAME_LENGTH {
            return Err(anyhow::Error::from(Error::client(format!(
                "API key name must be non-empty and at most {MAX_API_KEY_NAME_LENGTH} characters."
            ))));
        }
        Ok(())
    }

    fn validate_expires_at(expires_at: OffsetDateTime) -> anyhow::Result<()> {
        if expires_at <= OffsetDateTime::now_utc() {
            return Err(anyhow::Error::from(Error::client(
                "Expiration date must be in the future.",
            )));
        }
        Ok(())
    }
}

pub struct ApiKeysApiSystemExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> ApiKeysApiSystemExt<'a, DR, ET> {
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Validates an API key token: hashes it, looks it up in the database, and
    /// checks expiry. Returns the key if valid, `None` if not found or expired.
    pub async fn validate_api_key_token(&self, token: &str) -> anyhow::Result<Option<UserApiKey>> {
        let token_hash = hash_api_key_token(token);
        let Some(api_key) = self
            .api
            .db
            .api_keys()
            .get_user_api_key_by_hash(&token_hash)
            .await?
        else {
            return Ok(None);
        };

        if api_key
            .expires_at
            .is_some_and(|expires_at| expires_at <= OffsetDateTime::now_utc())
        {
            warn!(user.id = %api_key.user_id, api_key.id = %api_key.id, "API key has expired.");
            return Ok(None);
        }

        Ok(Some(api_key))
    }

    /// Best-effort update of `last_used_at` for an API key.
    pub async fn touch_api_key_last_used(&self, api_key_id: Uuid) {
        let now = OffsetDateTime::now_utc();
        if let Err(err) = self
            .api
            .db
            .api_keys()
            .update_api_key_last_used(api_key_id, now)
            .await
        {
            warn!(
                api_key.id = %api_key_id,
                "Failed to update API key last_used_at: {err:?}"
            );
        }
    }
}

/// Hashes a plaintext token string with SHA-256 (used during authentication).
pub fn hash_api_key_token(token: &str) -> Vec<u8> {
    openssl::hash::hash(MessageDigest::sha256(), token.as_bytes())
        .expect("SHA-256 hashing must not fail")
        .to_vec()
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with user API keys scoped to a specific user.
    pub fn api_keys<'a, 'u>(&'a self, user: &'u User) -> ApiKeysApiExt<'a, 'u, DR, ET> {
        ApiKeysApiExt::new(self, user)
    }

    /// Returns an unscoped API to work with user API keys.
    pub fn api_keys_system(&self) -> ApiKeysApiSystemExt<'_, DR, ET> {
        ApiKeysApiSystemExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiKeyCreateParams, ApiKeyRegenerateParams, ApiKeyUpdateParams};
    use crate::tests::{mock_api, mock_api_with_config, mock_config, mock_user, schema_example};
    use sqlx::PgPool;
    use time::OffsetDateTime;

    #[test]
    fn api_key_create_params_example_is_valid() {
        let example: ApiKeyCreateParams =
            serde_json::from_value(schema_example::<ApiKeyCreateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn api_key_update_params_example_is_valid() {
        let example: ApiKeyUpdateParams =
            serde_json::from_value(schema_example::<ApiKeyUpdateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn api_key_regenerate_params_example_is_valid() {
        let _example: ApiKeyRegenerateParams =
            serde_json::from_value(schema_example::<ApiKeyRegenerateParams>()).unwrap();
    }

    #[sqlx::test]
    async fn list_api_keys_returns_empty_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let keys = api.api_keys(&mock_user).list_api_keys().await?;
        assert!(keys.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn create_api_key_returns_plaintext(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let (key, plaintext) = api
            .api_keys(&mock_user)
            .create_api_key(ApiKeyCreateParams {
                name: "Test Key".into(),
                expires_at: None,
            })
            .await?;

        assert_eq!(key.name, "Test Key");
        assert!(plaintext.starts_with("su_ak_"));
        assert!(key.expires_at.is_none());
        assert!(key.last_used_at.is_none());

        let list = api.api_keys(&mock_user).list_api_keys().await?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Test Key");

        Ok(())
    }

    #[sqlx::test]
    async fn create_api_key_validates_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let api_keys = api.api_keys(&mock_user);

        let err = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "".into(),
                expires_at: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("API key name must"));

        let err = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "   ".into(),
                expires_at: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("API key name must"));

        let err = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "a".repeat(129),
                expires_at: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("API key name must"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_api_key_validates_expires_at(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .api_keys(&mock_user)
            .create_api_key(ApiKeyCreateParams {
                name: "Past".into(),
                expires_at: Some(OffsetDateTime::from_unix_timestamp(0)?),
            })
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Expiration date must be in the future")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_key_changes_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let api_keys = api.api_keys(&mock_user);
        let (created, _) = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Old".into(),
                expires_at: None,
            })
            .await?;

        let updated = api_keys
            .update_api_key(created.id, ApiKeyUpdateParams { name: "New".into() })
            .await?;
        assert_eq!(updated.name, "New");

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_key_rejects_duplicate_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let api_keys = api.api_keys(&mock_user);
        api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Existing".into(),
                expires_at: None,
            })
            .await?;
        let (second, _) = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Other".into(),
                expires_at: None,
            })
            .await?;

        let err = api_keys
            .update_api_key(
                second.id,
                ApiKeyUpdateParams {
                    name: "Existing".into(),
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_key_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .api_keys(&mock_user)
            .update_api_key(
                uuid::Uuid::now_v7(),
                ApiKeyUpdateParams { name: "X".into() },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn delete_api_key_removes_it(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let api_keys = api.api_keys(&mock_user);
        let (created, _) = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Delete Me".into(),
                expires_at: None,
            })
            .await?;
        assert_eq!(api_keys.list_api_keys().await?.len(), 1);

        let deleted = api_keys.delete_api_key(created.id).await?;
        assert_eq!(deleted.name, "Delete Me");
        assert!(api_keys.list_api_keys().await?.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn delete_api_key_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .api_keys(&mock_user)
            .delete_api_key(uuid::Uuid::now_v7())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn regenerate_api_key_invalidates_old_token(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let api_keys = api.api_keys(&mock_user);
        let (created, old_plaintext) = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Regen".into(),
                expires_at: None,
            })
            .await?;

        let (regenerated, new_plaintext) = api_keys
            .regenerate_api_key(created.id, ApiKeyRegenerateParams { expires_at: None })
            .await?;

        assert_eq!(regenerated.id, created.id);
        assert_eq!(regenerated.name, "Regen");
        assert_ne!(old_plaintext, new_plaintext);
        assert!(new_plaintext.starts_with("su_ak_"));
        assert!(regenerated.last_used_at.is_none());

        // Old token hash should no longer resolve.
        let old_hash = super::hash_api_key_token(&old_plaintext);
        let lookup = api
            .db
            .api_keys()
            .get_user_api_key_by_hash(&old_hash)
            .await?;
        assert!(lookup.is_none());

        // New token hash should resolve.
        let new_hash = super::hash_api_key_token(&new_plaintext);
        let lookup = api
            .db
            .api_keys()
            .get_user_api_key_by_hash(&new_hash)
            .await?;
        assert!(lookup.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn regenerate_api_key_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .api_keys(&mock_user)
            .regenerate_api_key(
                uuid::Uuid::now_v7(),
                ApiKeyRegenerateParams { expires_at: None },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_api_key_enforces_limit(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.max_user_api_keys = 2;
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let api_keys = api.api_keys(&mock_user);
        api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Key A".into(),
                expires_at: None,
            })
            .await?;
        api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Key B".into(),
                expires_at: None,
            })
            .await?;

        let err = api_keys
            .create_api_key(ApiKeyCreateParams {
                name: "Key C".into(),
                expires_at: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Maximum number of API keys (2)"));

        Ok(())
    }
}
