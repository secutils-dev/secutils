use crate::{
    database::Database,
    error::Error,
    users::{UserId, scripts::UserScript},
};
use sqlx::{error::ErrorKind as SqlxErrorKind, query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
struct RawUserScript {
    id: Uuid,
    user_id: Uuid,
    name: String,
    r#type: String,
    content: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl RawUserScript {
    fn into_user_script(self) -> anyhow::Result<UserScript> {
        use crate::users::scripts::UserScriptType;

        let script_type = match self.r#type.as_str() {
            "responder" => UserScriptType::Responder,
            "api_configurator" => UserScriptType::ApiConfigurator,
            "api_extractor" => UserScriptType::ApiExtractor,
            "page_extractor" => UserScriptType::PageExtractor,
            "universal" => UserScriptType::Universal,
            _ => anyhow::bail!("Unknown script type: {}", self.r#type),
        };

        Ok(UserScript {
            id: self.id,
            user_id: self.user_id.into(),
            name: self.name,
            script_type,
            content: self.content,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// Extends the primary database with user scripts CRUD methods.
impl Database {
    /// Lists all scripts for a user.
    pub async fn get_user_scripts(&self, user_id: UserId) -> anyhow::Result<Vec<UserScript>> {
        let raw: Vec<RawUserScript> = query_as!(
            RawUserScript,
            r#"
SELECT id, user_id, name, type, content, created_at, updated_at
FROM user_data_scripts
WHERE user_id = $1
ORDER BY name ASC
            "#,
            *user_id
        )
        .fetch_all(&self.pool)
        .await?;

        raw.into_iter()
            .map(|r| r.into_user_script())
            .collect::<Result<Vec<_>, _>>()
    }

    /// Lists scripts for a user matching the specified IDs.
    pub async fn bulk_get_user_scripts(
        &self,
        user_id: UserId,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<UserScript>> {
        let raw: Vec<RawUserScript> = query_as!(
            RawUserScript,
            r#"
SELECT id, user_id, name, type, content, created_at, updated_at
FROM user_data_scripts
WHERE user_id = $1 AND id = ANY($2)
ORDER BY name ASC
            "#,
            *user_id,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        raw.into_iter()
            .map(|r| r.into_user_script())
            .collect::<Result<Vec<_>, _>>()
    }

    /// Counts scripts for a user.
    pub async fn count_user_scripts(&self, user_id: UserId) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM user_data_scripts WHERE user_id = $1"#,
            *user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Gets a single script by id for a user.
    pub async fn get_user_script_by_id(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<UserScript>> {
        let raw: Option<RawUserScript> = query_as!(
            RawUserScript,
            r#"
SELECT id, user_id, name, type, content, created_at, updated_at
FROM user_data_scripts
WHERE user_id = $1 AND id = $2
            "#,
            *user_id,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        raw.map(|r| r.into_user_script()).transpose()
    }

    /// Inserts a new script. Returns the created UserScript.
    pub async fn insert_user_script(
        &self,
        user_id: UserId,
        name: &str,
        script_type: &str,
        content: &str,
    ) -> anyhow::Result<UserScript> {
        let id = Uuid::now_v7();
        let now = OffsetDateTime::now_utc();
        match query!(
            r#"
INSERT INTO user_data_scripts (id, user_id, name, type, content, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            id,
            *user_id,
            name,
            script_type,
            content,
            now,
            now
        )
        .execute(&self.pool)
        .await
        {
            Ok(_) => {}
            Err(err) => {
                let is_conflict = err
                    .as_database_error()
                    .map(|db_err| matches!(db_err.kind(), SqlxErrorKind::UniqueViolation))
                    .unwrap_or_default();
                return Err(if is_conflict {
                    anyhow::Error::from(Error::conflict(format!(
                        "A script with name '{name}' already exists."
                    )))
                } else {
                    err.into()
                });
            }
        }

        Ok(UserScript {
            id,
            user_id,
            name: name.to_string(),
            script_type: match script_type {
                "responder" => crate::users::scripts::UserScriptType::Responder,
                "api_configurator" => crate::users::scripts::UserScriptType::ApiConfigurator,
                "api_extractor" => crate::users::scripts::UserScriptType::ApiExtractor,
                "page_extractor" => crate::users::scripts::UserScriptType::PageExtractor,
                "universal" => crate::users::scripts::UserScriptType::Universal,
                _ => anyhow::bail!("Unknown script type: {}", script_type),
            },
            content: content.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Updates the content of an existing script by user_id and id.
    pub async fn update_user_script(
        &self,
        user_id: UserId,
        id: Uuid,
        content: &str,
    ) -> anyhow::Result<Option<UserScript>> {
        let now = OffsetDateTime::now_utc();
        let raw: Option<RawUserScript> = query_as!(
            RawUserScript,
            r#"
UPDATE user_data_scripts SET content = $1, updated_at = $2
WHERE user_id = $3 AND id = $4
RETURNING id, user_id, name, type, content, created_at, updated_at
            "#,
            content,
            now,
            *user_id,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        raw.map(|r| r.into_user_script()).transpose()
    }

    /// Removes a script by user_id and id.
    pub async fn remove_user_script(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<UserScript>> {
        let raw: Option<RawUserScript> = query_as!(
            RawUserScript,
            r#"
DELETE FROM user_data_scripts
WHERE user_id = $1 AND id = $2
RETURNING id as "id!", user_id as "user_id!", name as "name!", type as "type!", content as "content!", created_at as "created_at!", updated_at as "updated_at!"
            "#,
            *user_id,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        raw.map(|r| r.into_user_script()).transpose()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error,
        tests::{mock_user, mock_user_with_id},
        users::scripts::{UserScriptType, database_ext::RawUserScript},
    };
    use actix_web::{ResponseError, http::StatusCode};
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_insert_and_list_scripts(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(db.get_user_scripts(user.id).await?.is_empty());
        assert_eq!(db.count_user_scripts(user.id).await?, 0);

        let script = db
            .insert_user_script(user.id, "test_script", "responder", "console.log('hello');")
            .await?;
        assert_eq!(script.name, "test_script");
        assert_eq!(script.script_type, UserScriptType::Responder);
        assert_eq!(script.content, "console.log('hello');");
        assert_eq!(script.user_id, user.id);

        db.insert_user_script(user.id, "another_script", "api_extractor", "return data;")
            .await?;

        let scripts = db.get_user_scripts(user.id).await?;
        assert_eq!(scripts.len(), 2);
        assert_eq!(scripts[0].name, "another_script");
        assert_eq!(scripts[1].name, "test_script");
        assert_eq!(db.count_user_scripts(user.id).await?, 2);

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_script_by_id(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(
            db.get_user_script_by_id(user.id, uuid!("00000000-0000-0000-0000-000000000099"))
                .await?
                .is_none()
        );

        let script = db
            .insert_user_script(user.id, "my_script", "responder", "content here")
            .await?;

        let fetched = db.get_user_script_by_id(user.id, script.id).await?.unwrap();
        assert_eq!(fetched.name, "my_script");
        assert_eq!(fetched.content, "content here");

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_script(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(
            db.update_user_script(
                user.id,
                uuid!("00000000-0000-0000-0000-000000000099"),
                "new content"
            )
            .await?
            .is_none()
        );

        let original = db
            .insert_user_script(user.id, "my_script", "responder", "old content")
            .await?;

        let updated = db
            .update_user_script(user.id, original.id, "new content")
            .await?
            .unwrap();
        assert_eq!(updated.name, "my_script");
        assert_eq!(updated.id, original.id);
        assert_eq!(updated.content, "new content");
        assert!(updated.updated_at >= original.updated_at);

        // Verify the update persisted
        let script = db
            .get_user_script_by_id(user.id, original.id)
            .await?
            .unwrap();
        assert_eq!(script.content, "new content");

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_script(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(
            db.remove_user_script(user.id, uuid!("00000000-0000-0000-0000-000000000099"))
                .await?
                .is_none()
        );

        let inserted = db
            .insert_user_script(user.id, "to_delete", "responder", "content")
            .await?;

        let removed = db.remove_user_script(user.id, inserted.id).await?.unwrap();
        assert_eq!(removed.name, "to_delete");
        assert!(db.get_user_scripts(user.id).await?.is_empty());
        assert!(db.remove_user_script(user.id, inserted.id).await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn scripts_are_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        db.insert_user_script(user_a.id, "shared_name", "responder", "user_a_content")
            .await?;
        db.insert_user_script(user_b.id, "shared_name", "responder", "user_b_content")
            .await?;

        let scripts_a = db.get_user_scripts(user_a.id).await?;
        let scripts_b = db.get_user_scripts(user_b.id).await?;
        assert_eq!(scripts_a.len(), 1);
        assert_eq!(scripts_b.len(), 1);
        assert_eq!(scripts_a[0].content, "user_a_content");
        assert_eq!(scripts_b[0].content, "user_b_content");

        assert_eq!(db.count_user_scripts(user_a.id).await?, 1);
        assert_eq!(db.count_user_scripts(user_b.id).await?, 1);

        Ok(())
    }

    #[sqlx::test]
    async fn duplicate_name_returns_conflict_error(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_script(user.id, "my_script", "responder", "content1")
            .await?;
        let err = db
            .insert_user_script(user.id, "my_script", "responder", "content2")
            .await
            .unwrap_err();

        let typed = err.downcast::<Error>().unwrap();
        assert_eq!(typed.status_code(), StatusCode::CONFLICT);
        assert!(typed.root_cause.to_string().contains("my_script"));

        Ok(())
    }

    #[sqlx::test]
    async fn cascade_delete_on_user_removal(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_script(user.id, "script1", "responder", "content")
            .await?;
        assert_eq!(db.count_user_scripts(user.id).await?, 1);

        db.remove_user_by_email(&user.email).await?;
        assert_eq!(db.count_user_scripts(user.id).await?, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_scripts_empty(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts = db.bulk_get_user_scripts(user.id, &[]).await?;
        assert!(scripts.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_scripts_returns_matching(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let script_a = db
            .insert_user_script(user.id, "alpha_script", "responder", "content_a")
            .await?;
        let script_b = db
            .insert_user_script(user.id, "beta_script", "api_extractor", "content_b")
            .await?;
        db.insert_user_script(user.id, "gamma_script", "universal", "content_c")
            .await?;

        let scripts = db
            .bulk_get_user_scripts(user.id, &[script_a.id, script_b.id])
            .await?;
        assert_eq!(scripts.len(), 2);
        assert_eq!(scripts[0].name, "alpha_script");
        assert_eq!(scripts[1].name, "beta_script");

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_scripts_ignores_non_existent(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let script = db
            .insert_user_script(user.id, "my_script", "responder", "content")
            .await?;

        let scripts = db
            .bulk_get_user_scripts(
                user.id,
                &[script.id, uuid!("00000000-0000-0000-0000-000000000099")],
            )
            .await?;
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].name, "my_script");

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_scripts_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let script_a = db
            .insert_user_script(user_a.id, "shared_name", "responder", "content_a")
            .await?;
        db.insert_user_script(user_b.id, "shared_name", "responder", "content_b")
            .await?;

        let scripts = db.bulk_get_user_scripts(user_b.id, &[script_a.id]).await?;
        assert!(scripts.is_empty());

        Ok(())
    }

    #[test]
    fn raw_script_into_user_script_handles_all_types() -> anyhow::Result<()> {
        let test_cases = vec![
            ("responder", UserScriptType::Responder),
            ("api_configurator", UserScriptType::ApiConfigurator),
            ("api_extractor", UserScriptType::ApiExtractor),
            ("page_extractor", UserScriptType::PageExtractor),
            ("universal", UserScriptType::Universal),
        ];

        for (type_str, expected_enum) in test_cases {
            let raw = RawUserScript {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                user_id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "test".to_string(),
                r#type: type_str.to_string(),
                content: "test content".to_string(),
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
            };

            let script = raw.into_user_script()?;
            assert_eq!(script.script_type, expected_enum);
        }

        // Test unknown type error
        let raw_unknown = RawUserScript {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            user_id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "test".to_string(),
            r#type: "unknown_type".to_string(),
            content: "test content".to_string(),
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        assert!(raw_unknown.into_user_script().is_err());

        Ok(())
    }
}
