use crate::{
    database::Database,
    users::{UserId, secrets::UserSecret},
};
use sqlx::{query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
struct RawUserSecret {
    id: Uuid,
    user_id: Uuid,
    name: String,
    value: Vec<u8>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl RawUserSecret {
    fn into_user_secret(self, include_value: bool) -> UserSecret {
        UserSecret {
            id: self.id,
            user_id: self.user_id.into(),
            name: self.name,
            encrypted_value: if include_value {
                Some(self.value)
            } else {
                None
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Extends the primary database with user secrets CRUD methods.
impl Database {
    /// Lists all secrets for a user. When `include_values` is true, each
    /// `UserSecret.encrypted_value` is populated (for internal decryption use).
    pub async fn get_user_secrets(
        &self,
        user_id: UserId,
        include_values: bool,
    ) -> anyhow::Result<Vec<UserSecret>> {
        let raw: Vec<RawUserSecret> = query_as!(
            RawUserSecret,
            r#"
SELECT id, user_id, name, value, created_at, updated_at
FROM user_data_secrets
WHERE user_id = $1
ORDER BY name ASC
            "#,
            *user_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(raw
            .into_iter()
            .map(|r| r.into_user_secret(include_values))
            .collect())
    }

    /// Counts secrets for a user.
    pub async fn count_user_secrets(&self, user_id: UserId) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM user_data_secrets WHERE user_id = $1"#,
            *user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Inserts a new secret. Returns the created UserSecret metadata.
    pub async fn insert_user_secret(
        &self,
        user_id: UserId,
        name: &str,
        encrypted_value: &[u8],
    ) -> anyhow::Result<UserSecret> {
        let id = Uuid::now_v7();
        let now = OffsetDateTime::now_utc();
        query!(
            r#"
INSERT INTO user_data_secrets (id, user_id, name, value, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            id,
            *user_id,
            name,
            encrypted_value,
            now,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(UserSecret {
            id,
            user_id,
            name: name.to_string(),
            encrypted_value: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Updates the encrypted value of an existing secret by user_id and name.
    pub async fn update_user_secret(
        &self,
        user_id: UserId,
        name: &str,
        encrypted_value: &[u8],
    ) -> anyhow::Result<Option<UserSecret>> {
        let now = OffsetDateTime::now_utc();
        let raw: Option<RawUserSecret> = query_as!(
            RawUserSecret,
            r#"
UPDATE user_data_secrets SET value = $1, updated_at = $2
WHERE user_id = $3 AND name = $4
RETURNING id, user_id, name, value, created_at, updated_at
            "#,
            encrypted_value,
            now,
            *user_id,
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(raw.map(|r| r.into_user_secret(false)))
    }

    /// Removes a secret by user_id and name.
    pub async fn remove_user_secret(
        &self,
        user_id: UserId,
        name: &str,
    ) -> anyhow::Result<Option<UserSecret>> {
        let raw: Option<RawUserSecret> = query_as!(
            RawUserSecret,
            r#"
DELETE FROM user_data_secrets
WHERE user_id = $1 AND name = $2
RETURNING id as "id!", user_id as "user_id!", name as "name!", value as "value!", created_at as "created_at!", updated_at as "updated_at!"
            "#,
            *user_id,
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(raw.map(|r| r.into_user_secret(false)))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        tests::{mock_user, mock_user_with_id},
    };
    use sqlx::PgPool;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_insert_and_list_secrets(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(db.get_user_secrets(user.id, false).await?.is_empty());
        assert_eq!(db.count_user_secrets(user.id).await?, 0);

        let secret = db
            .insert_user_secret(user.id, "API_KEY", b"encrypted-value-1")
            .await?;
        assert_eq!(secret.name, "API_KEY");
        assert_eq!(secret.user_id, user.id);
        assert!(secret.encrypted_value.is_none());

        db.insert_user_secret(user.id, "DB_PASSWORD", b"encrypted-value-2")
            .await?;

        let secrets = db.get_user_secrets(user.id, false).await?;
        assert_eq!(secrets.len(), 2);
        assert_eq!(secrets[0].name, "API_KEY");
        assert!(secrets[0].encrypted_value.is_none());
        assert_eq!(secrets[1].name, "DB_PASSWORD");
        assert_eq!(db.count_user_secrets(user.id).await?, 2);

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_secrets_with_values(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_secret(user.id, "KEY_A", b"val-a").await?;
        db.insert_user_secret(user.id, "KEY_B", b"val-b").await?;

        let secrets = db.get_user_secrets(user.id, true).await?;
        assert_eq!(secrets.len(), 2);
        assert_eq!(secrets[0].name, "KEY_A");
        assert_eq!(
            secrets[0].encrypted_value.as_deref(),
            Some(b"val-a".as_slice())
        );
        assert_eq!(secrets[1].name, "KEY_B");
        assert_eq!(
            secrets[1].encrypted_value.as_deref(),
            Some(b"val-b".as_slice())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_secret(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(
            db.update_user_secret(user.id, "API_KEY", b"new-val")
                .await?
                .is_none()
        );

        let original = db
            .insert_user_secret(user.id, "API_KEY", b"old-val")
            .await?;

        let updated = db
            .update_user_secret(user.id, "API_KEY", b"new-val")
            .await?
            .unwrap();
        assert_eq!(updated.name, "API_KEY");
        assert_eq!(updated.id, original.id);
        assert!(updated.updated_at >= original.updated_at);

        let secrets = db.get_user_secrets(user.id, true).await?;
        let secret = secrets.iter().find(|s| s.name == "API_KEY").unwrap();
        assert_eq!(
            secret.encrypted_value.as_deref(),
            Some(b"new-val".as_slice())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_secret(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(db.remove_user_secret(user.id, "API_KEY").await?.is_none());

        db.insert_user_secret(user.id, "API_KEY", b"enc-val")
            .await?;

        let removed = db.remove_user_secret(user.id, "API_KEY").await?.unwrap();
        assert_eq!(removed.name, "API_KEY");
        assert!(db.get_user_secrets(user.id, false).await?.is_empty());
        assert!(db.remove_user_secret(user.id, "API_KEY").await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn secrets_are_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        db.insert_user_secret(user_a.id, "SHARED_NAME", b"val-a")
            .await?;
        db.insert_user_secret(user_b.id, "SHARED_NAME", b"val-b")
            .await?;

        let secrets_a = db.get_user_secrets(user_a.id, true).await?;
        let secrets_b = db.get_user_secrets(user_b.id, true).await?;
        assert_eq!(
            secrets_a[0].encrypted_value.as_deref(),
            Some(b"val-a".as_slice())
        );
        assert_eq!(
            secrets_b[0].encrypted_value.as_deref(),
            Some(b"val-b".as_slice())
        );

        assert_eq!(db.count_user_secrets(user_a.id).await?, 1);
        assert_eq!(db.count_user_secrets(user_b.id).await?, 1);

        Ok(())
    }

    #[sqlx::test]
    async fn duplicate_name_rejected(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_secret(user.id, "API_KEY", b"val-1").await?;
        assert!(
            db.insert_user_secret(user.id, "API_KEY", b"val-2")
                .await
                .is_err()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn cascade_delete_on_user_removal(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_secret(user.id, "API_KEY", b"val").await?;
        assert_eq!(db.count_user_secrets(user.id).await?, 1);

        db.remove_user_by_email(&user.email).await?;
        assert_eq!(db.count_user_secrets(user.id).await?, 0);

        Ok(())
    }
}
