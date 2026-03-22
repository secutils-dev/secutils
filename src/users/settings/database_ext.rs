use crate::{
    database::Database,
    users::{UserId, settings::UserSettings},
};
use anyhow::Context;
use sqlx::{query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
#[allow(dead_code)]
struct RawUserSettings {
    user_id: Uuid,
    value: Vec<u8>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl RawUserSettings {
    fn try_into_user_settings(self) -> anyhow::Result<UserSettings> {
        serde_json::from_slice(&self.value)
            .with_context(|| "Cannot deserialize user settings value")
    }
}

/// Extends the primary database with user settings CRUD methods.
impl Database {
    /// Retrieves user settings for the specified user.
    pub async fn get_user_settings(&self, user_id: UserId) -> anyhow::Result<Option<UserSettings>> {
        query_as!(
            RawUserSettings,
            r#"
SELECT user_id, value, created_at, updated_at
FROM user_settings
WHERE user_id = $1
            "#,
            *user_id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|raw| raw.try_into_user_settings())
        .transpose()
    }

    /// Inserts or updates user settings for the specified user.
    pub async fn upsert_user_settings(
        &self,
        user_id: UserId,
        settings: &UserSettings,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let value =
            serde_json::to_vec(settings).with_context(|| "Cannot serialize user settings value")?;
        query!(
            r#"
INSERT INTO user_settings (user_id, value, created_at, updated_at)
VALUES ($1, $2, $3, $3)
ON CONFLICT(user_id) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at
            "#,
            *user_id,
            value,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user settings for the specified user.
    pub async fn remove_user_settings(&self, user_id: UserId) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_settings
WHERE user_id = $1
            "#,
            *user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{database::Database, tests::mock_user, users::settings::UserSettings};
    use serde_json::json;
    use sqlx::PgPool;
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    #[sqlx::test]
    async fn can_get_settings_returns_none_when_empty(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(db.get_user_settings(user.id).await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_upsert_and_get_settings(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let settings = UserSettings(
            [("common.uiTheme".to_string(), json!("dark"))]
                .into_iter()
                .collect(),
        );
        db.upsert_user_settings(
            user.id,
            &settings,
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .await?;

        let retrieved = db.get_user_settings(user.id).await?.unwrap();
        assert_eq!(retrieved, settings);

        // Update settings.
        let updated_settings = UserSettings(
            [
                ("common.uiTheme".to_string(), json!("light")),
                ("common.showOnlyFavorites".to_string(), json!(true)),
            ]
            .into_iter()
            .collect(),
        );
        db.upsert_user_settings(
            user.id,
            &updated_settings,
            OffsetDateTime::from_unix_timestamp(946720801)?,
        )
        .await?;

        let retrieved = db.get_user_settings(user.id).await?.unwrap();
        assert_eq!(retrieved, updated_settings);

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_settings(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let settings = UserSettings(
            [("common.uiTheme".to_string(), json!("dark"))]
                .into_iter()
                .collect(),
        );
        db.upsert_user_settings(
            user.id,
            &settings,
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .await?;
        assert!(db.get_user_settings(user.id).await?.is_some());

        db.remove_user_settings(user.id).await?;
        assert!(db.get_user_settings(user.id).await?.is_none());

        // Removing again is a no-op.
        db.remove_user_settings(user.id).await?;

        Ok(())
    }

    #[sqlx::test]
    async fn settings_deleted_on_user_removal(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let settings = UserSettings(BTreeMap::new());
        db.upsert_user_settings(user.id, &settings, OffsetDateTime::now_utc())
            .await?;
        assert!(db.get_user_settings(user.id).await?.is_some());

        db.remove_user_by_email(&user.email).await?;
        assert!(db.get_user_settings(user.id).await?.is_none());

        Ok(())
    }
}
