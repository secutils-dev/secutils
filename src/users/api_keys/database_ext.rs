use crate::{
    database::Database,
    error::Error,
    users::{UserId, api_keys::UserApiKey},
};
use sqlx::{Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
struct RawUserApiKey {
    id: Uuid,
    user_id: Uuid,
    name: String,
    token_hash: Vec<u8>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    expires_at: Option<OffsetDateTime>,
    last_used_at: Option<OffsetDateTime>,
}

impl From<RawUserApiKey> for UserApiKey {
    fn from(raw: RawUserApiKey) -> Self {
        Self {
            id: raw.id,
            user_id: raw.user_id.into(),
            name: raw.name,
            token_hash: raw.token_hash,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            expires_at: raw.expires_at,
            last_used_at: raw.last_used_at,
        }
    }
}

pub struct ApiKeysDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
}

impl<'pool> ApiKeysDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Returns the number of API keys owned by a user.
    pub async fn count_user_api_keys(&self, user_id: UserId) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM user_api_keys WHERE user_id = $1"#,
            *user_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Lists all API keys for a user (ordered by name).
    pub async fn get_user_api_keys(&self, user_id: UserId) -> anyhow::Result<Vec<UserApiKey>> {
        let raw: Vec<RawUserApiKey> = query_as!(
            RawUserApiKey,
            r#"
SELECT id, user_id, name, token_hash, created_at, updated_at, expires_at, last_used_at
FROM user_api_keys
WHERE user_id = $1
ORDER BY name ASC
            "#,
            *user_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(raw.into_iter().map(UserApiKey::from).collect())
    }

    /// Gets a single API key by user_id and id.
    #[cfg(test)]
    pub async fn get_user_api_key(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<UserApiKey>> {
        let raw: Option<RawUserApiKey> = query_as!(
            RawUserApiKey,
            r#"
SELECT id, user_id, name, token_hash, created_at, updated_at, expires_at, last_used_at
FROM user_api_keys
WHERE user_id = $1 AND id = $2
            "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(raw.map(UserApiKey::from))
    }

    /// Looks up an API key by its SHA-256 token hash (used during authentication).
    pub async fn get_user_api_key_by_hash(
        &self,
        token_hash: &[u8],
    ) -> anyhow::Result<Option<UserApiKey>> {
        let raw: Option<RawUserApiKey> = query_as!(
            RawUserApiKey,
            r#"
SELECT id, user_id, name, token_hash, created_at, updated_at, expires_at, last_used_at
FROM user_api_keys
WHERE token_hash = $1
            "#,
            token_hash
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(raw.map(UserApiKey::from))
    }

    /// Inserts a new API key.
    pub async fn insert_user_api_key(
        &self,
        user_id: UserId,
        api_key: &UserApiKey,
    ) -> anyhow::Result<()> {
        match query!(
            r#"
INSERT INTO user_api_keys (id, user_id, name, token_hash, created_at, updated_at, expires_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            api_key.id,
            *user_id,
            api_key.name,
            api_key.token_hash,
            api_key.created_at,
            api_key.updated_at,
            api_key.expires_at
        )
        .execute(self.pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(err)
                if err.as_database_error().is_some_and(|db_err| {
                    matches!(db_err.kind(), SqlxErrorKind::UniqueViolation)
                }) =>
            {
                Err(anyhow::Error::from(Error::conflict(format!(
                    "An API key with name '{}' already exists.",
                    api_key.name
                ))))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Updates only the name (and updated_at) of an API key.
    pub async fn update_user_api_key_name(
        &self,
        user_id: UserId,
        id: Uuid,
        name: &str,
        updated_at: OffsetDateTime,
    ) -> anyhow::Result<Option<UserApiKey>> {
        let raw: Option<RawUserApiKey> = query_as!(
            RawUserApiKey,
            r#"
UPDATE user_api_keys SET name = $1, updated_at = $2
WHERE user_id = $3 AND id = $4
RETURNING id, user_id, name, token_hash, created_at, updated_at, expires_at, last_used_at
            "#,
            name,
            updated_at,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(raw.map(UserApiKey::from))
    }

    /// Replaces the token hash and expires_at (regeneration).
    pub async fn update_user_api_key_token(
        &self,
        user_id: UserId,
        id: Uuid,
        token_hash: &[u8],
        expires_at: Option<OffsetDateTime>,
        updated_at: OffsetDateTime,
    ) -> anyhow::Result<Option<UserApiKey>> {
        let raw: Option<RawUserApiKey> = query_as!(
            RawUserApiKey,
            r#"
UPDATE user_api_keys SET token_hash = $1, expires_at = $2, updated_at = $3, last_used_at = NULL
WHERE user_id = $4 AND id = $5
RETURNING id, user_id, name, token_hash, created_at, updated_at, expires_at, last_used_at
            "#,
            token_hash,
            expires_at,
            updated_at,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(raw.map(UserApiKey::from))
    }

    /// Removes an API key by user_id and id.
    pub async fn remove_user_api_key(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<UserApiKey>> {
        let raw: Option<RawUserApiKey> = query_as!(
            RawUserApiKey,
            r#"
DELETE FROM user_api_keys
WHERE user_id = $1 AND id = $2
RETURNING id AS "id!", user_id AS "user_id!", name AS "name!", token_hash AS "token_hash!",
          created_at AS "created_at!", updated_at AS "updated_at!", expires_at, last_used_at
            "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(raw.map(UserApiKey::from))
    }

    /// Updates last_used_at for an API key (called during authentication).
    pub async fn update_api_key_last_used(
        &self,
        id: Uuid,
        timestamp: OffsetDateTime,
    ) -> anyhow::Result<()> {
        query!(
            "UPDATE user_api_keys SET last_used_at = $1 WHERE id = $2",
            timestamp,
            id
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }
}

impl Database {
    pub fn api_keys(&self) -> ApiKeysDatabaseExt<'_> {
        ApiKeysDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error,
        tests::{mock_user, mock_user_with_id},
        users::api_keys::UserApiKey,
    };
    use actix_web::{ResponseError, http::StatusCode};
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::{Uuid, uuid};

    fn mock_api_key(id: Uuid, name: &str, token_hash: &[u8]) -> UserApiKey {
        let now = OffsetDateTime::now_utc();
        UserApiKey {
            id,
            user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            name: name.to_string(),
            token_hash: token_hash.to_vec(),
            created_at: now,
            updated_at: now,
            expires_at: None,
            last_used_at: None,
        }
    }

    #[sqlx::test]
    async fn can_insert_and_list_api_keys(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        assert!(api_keys_db.get_user_api_keys(user.id).await?.is_empty());

        let key_a = mock_api_key(Uuid::now_v7(), "CI Token", b"hash-a");
        api_keys_db.insert_user_api_key(user.id, &key_a).await?;

        let key_b = mock_api_key(Uuid::now_v7(), "Dev Token", b"hash-b");
        api_keys_db.insert_user_api_key(user.id, &key_b).await?;

        let keys = api_keys_db.get_user_api_keys(user.id).await?;
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].name, "CI Token");
        assert_eq!(keys[1].name, "Dev Token");

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_api_key_by_id(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "My Key", b"hash-get");
        api_keys_db.insert_user_api_key(user.id, &key).await?;

        let fetched = api_keys_db.get_user_api_key(user.id, key.id).await?;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "My Key");

        let missing = api_keys_db
            .get_user_api_key(user.id, Uuid::now_v7())
            .await?;
        assert!(missing.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_api_key_by_hash(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "Hash Lookup", b"unique-hash");
        api_keys_db.insert_user_api_key(user.id, &key).await?;

        let fetched = api_keys_db.get_user_api_key_by_hash(b"unique-hash").await?;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Hash Lookup");

        let missing = api_keys_db.get_user_api_key_by_hash(b"nonexistent").await?;
        assert!(missing.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_api_key_name(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "Old Name", b"hash-rename");
        api_keys_db.insert_user_api_key(user.id, &key).await?;

        let updated = api_keys_db
            .update_user_api_key_name(user.id, key.id, "New Name", OffsetDateTime::now_utc())
            .await?;
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().name, "New Name");

        let not_found = api_keys_db
            .update_user_api_key_name(user.id, Uuid::now_v7(), "X", OffsetDateTime::now_utc())
            .await?;
        assert!(not_found.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_api_key_token(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "Regen", b"old-hash");
        api_keys_db.insert_user_api_key(user.id, &key).await?;

        let updated = api_keys_db
            .update_user_api_key_token(
                user.id,
                key.id,
                b"new-hash",
                None,
                OffsetDateTime::now_utc(),
            )
            .await?;
        assert!(updated.is_some());
        let updated = updated.unwrap();
        assert_eq!(updated.token_hash, b"new-hash");
        assert!(updated.last_used_at.is_none());

        let old_lookup = api_keys_db.get_user_api_key_by_hash(b"old-hash").await?;
        assert!(old_lookup.is_none());

        let new_lookup = api_keys_db.get_user_api_key_by_hash(b"new-hash").await?;
        assert!(new_lookup.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_api_key(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "To Delete", b"hash-del");
        api_keys_db.insert_user_api_key(user.id, &key).await?;

        let removed = api_keys_db.remove_user_api_key(user.id, key.id).await?;
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "To Delete");

        assert!(api_keys_db.get_user_api_keys(user.id).await?.is_empty());

        let again = api_keys_db.remove_user_api_key(user.id, key.id).await?;
        assert!(again.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_last_used(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "Active", b"hash-used");
        api_keys_db.insert_user_api_key(user.id, &key).await?;

        let now = OffsetDateTime::now_utc();
        api_keys_db.update_api_key_last_used(key.id, now).await?;

        let fetched = api_keys_db
            .get_user_api_key(user.id, key.id)
            .await?
            .unwrap();
        assert!(fetched.last_used_at.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn api_keys_are_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let api_keys_db = db.api_keys();
        let key_a = mock_api_key(Uuid::now_v7(), "Same Name", b"hash-user-a");
        api_keys_db.insert_user_api_key(user_a.id, &key_a).await?;

        let key_b = mock_api_key(Uuid::now_v7(), "Same Name", b"hash-user-b");
        api_keys_db.insert_user_api_key(user_b.id, &key_b).await?;

        assert_eq!(api_keys_db.get_user_api_keys(user_a.id).await?.len(), 1);
        assert_eq!(api_keys_db.get_user_api_keys(user_b.id).await?.len(), 1);

        assert!(
            api_keys_db
                .get_user_api_key(user_b.id, key_a.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn duplicate_name_returns_conflict(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key_a = mock_api_key(Uuid::now_v7(), "Dup", b"hash-dup-a");
        api_keys_db.insert_user_api_key(user.id, &key_a).await?;

        let key_b = mock_api_key(Uuid::now_v7(), "Dup", b"hash-dup-b");
        let err = api_keys_db
            .insert_user_api_key(user.id, &key_b)
            .await
            .unwrap_err();
        let typed = err.downcast::<Error>().unwrap();
        assert_eq!(typed.status_code(), StatusCode::CONFLICT);

        Ok(())
    }

    #[sqlx::test]
    async fn cascade_delete_on_user_removal(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let api_keys_db = db.api_keys();
        let key = mock_api_key(Uuid::now_v7(), "Cascade", b"hash-cascade");
        api_keys_db.insert_user_api_key(user.id, &key).await?;
        assert_eq!(api_keys_db.get_user_api_keys(user.id).await?.len(), 1);

        db.remove_user_by_email(&user.email).await?;
        assert!(api_keys_db.get_user_api_keys(user.id).await?.is_empty());

        Ok(())
    }
}
