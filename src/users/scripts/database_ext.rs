use crate::{
    database::Database,
    error::Error,
    users::{EntityTag, RawEntityTag, UserId, group_entity_tags, scripts::UserScript},
};
use sqlx::{Acquire, Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub(super) struct RawUserScript {
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

        Ok(UserScript {
            id: self.id,
            user_id: self.user_id.into(),
            name: self.name,
            script_type: UserScriptType::from_str(&self.r#type)?,
            content: self.content,
            tags: vec![],
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// A database extension for script-related operations.
pub struct ScriptsDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
}

impl<'pool> ScriptsDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>) -> Self {
        Self { pool }
    }

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
        .fetch_all(self.pool)
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
        .fetch_all(self.pool)
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
        .fetch_one(self.pool)
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
        .fetch_optional(self.pool)
        .await?;

        raw.map(|r| r.into_user_script()).transpose()
    }

    /// Inserts a new script (and associated tags). Returns resolved tags.
    pub async fn insert_user_script(
        &self,
        user_id: UserId,
        script: &UserScript,
    ) -> anyhow::Result<Vec<EntityTag>> {
        let script_type = script.script_type.as_str();
        let mut tx = self.pool.begin().await?;
        match query!(
            r#"
INSERT INTO user_data_scripts (id, user_id, name, type, content, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            script.id,
            *user_id,
            script.name,
            script_type,
            script.content,
            script.created_at,
            script.updated_at
        )
        .execute(&mut *tx)
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
                        "A script with name '{}' already exists.",
                        script.name
                    )))
                } else {
                    err.into()
                });
            }
        }

        let tags = if script.tags.is_empty() {
            vec![]
        } else {
            Self::set_script_tags(
                &mut *tx,
                script.id,
                &script.tags.iter().map(|t| t.id).collect::<Vec<_>>(),
            )
            .await?
        };

        tx.commit().await?;
        Ok(tags)
    }

    /// Updates the content of an existing script (and optionally associated tags). Returns updated
    /// tags if tag_ids was provided, or None if tags were not changed.
    pub async fn update_user_script(
        &self,
        user_id: UserId,
        script: &UserScript,
        tag_ids: Option<Vec<Uuid>>,
    ) -> anyhow::Result<Option<Vec<EntityTag>>> {
        let mut tx = self.pool.begin().await?;
        let result = query!(
            r#"
UPDATE user_data_scripts SET content = $1, updated_at = $2
WHERE user_id = $3 AND id = $4
            "#,
            script.content,
            script.updated_at,
            *user_id,
            script.id
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::Error::from(Error::not_found(format!(
                "Script '{}' not found.",
                script.id
            ))));
        }

        let updated_tags = if let Some(ref tag_ids) = tag_ids {
            Some(Self::set_script_tags(&mut *tx, script.id, tag_ids).await?)
        } else {
            None
        };

        tx.commit().await?;
        Ok(updated_tags)
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
        .fetch_optional(self.pool)
        .await?;

        raw.map(|r| r.into_user_script()).transpose()
    }

    /// Fetches tags for a batch of scripts.
    pub async fn get_script_tags(
        &self,
        entity_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, Vec<EntityTag>>> {
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = query_as!(
            RawEntityTag,
            r#"
SELECT jt.script_id AS entity_id, t.id, t.name, t.color
FROM user_data_scripts_tags jt
JOIN user_tags t ON jt.tag_id = t.id
WHERE jt.script_id = ANY($1)
ORDER BY t.name ASC
            "#,
            entity_ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(group_entity_tags(rows))
    }

    async fn set_script_tags<'a>(
        executor: impl Acquire<'a, Database = Postgres>,
        entity_id: Uuid,
        tag_ids: &[Uuid],
    ) -> anyhow::Result<Vec<EntityTag>> {
        let mut conn = executor.acquire().await?;
        query!(
            "DELETE FROM user_data_scripts_tags WHERE script_id = $1",
            entity_id
        )
        .execute(&mut *conn)
        .await?;

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        // Insert new associations and return resolved tags in a single round-trip.
        Ok(query_as!(
            EntityTag,
            r#"
WITH inserted AS (
    INSERT INTO user_data_scripts_tags (script_id, tag_id)
    SELECT $1, unnest($2::uuid[])
    RETURNING script_id, tag_id
)
SELECT t.id, t.name, t.color
FROM inserted i
JOIN user_tags t ON i.tag_id = t.id
ORDER BY t.name ASC
            "#,
            entity_id,
            tag_ids
        )
        .fetch_all(&mut *conn)
        .await?)
    }
}

impl Database {
    /// Returns a database extension for script-related operations.
    pub fn scripts(&self) -> ScriptsDatabaseExt<'_> {
        ScriptsDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error,
        tests::{mock_user, mock_user_with_id},
        users::{
            EntityTag,
            scripts::{UserScript, UserScriptType, database_ext::RawUserScript},
        },
    };
    use actix_web::{ResponseError, http::StatusCode};
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::{Uuid, uuid};

    fn mock_script(id: Uuid, name: &str, script_type: &str) -> anyhow::Result<UserScript> {
        let now = OffsetDateTime::from_unix_timestamp(946720800)?;
        Ok(UserScript {
            id,
            user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            name: name.to_string(),
            script_type: UserScriptType::from_str(script_type)?,
            content: "console.log('hello');".to_string(),
            tags: vec![],
            created_at: now,
            updated_at: now,
        })
    }

    fn mock_script_with_tags(id: Uuid, name: &str, tag_ids: &[Uuid]) -> anyhow::Result<UserScript> {
        let mut script = mock_script(id, name, "responder")?;
        script.tags = tag_ids.iter().map(|id| EntityTag::from(*id)).collect();
        Ok(script)
    }

    #[sqlx::test]
    async fn can_insert_and_list_scripts(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        assert!(scripts_db.get_user_scripts(user.id).await?.is_empty());
        assert_eq!(scripts_db.count_user_scripts(user.id).await?, 0);

        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "test_script",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let script2 = mock_script(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "another_script",
            "api_extractor",
        )?;
        scripts_db.insert_user_script(user.id, &script2).await?;

        let scripts = scripts_db.get_user_scripts(user.id).await?;
        assert_eq!(scripts.len(), 2);
        assert_eq!(scripts[0].name, "another_script");
        assert_eq!(scripts[1].name, "test_script");
        assert_eq!(scripts_db.count_user_scripts(user.id).await?, 2);

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_script_by_id(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        assert!(
            scripts_db
                .get_user_script_by_id(user.id, uuid!("00000000-0000-0000-0000-000000000099"))
                .await?
                .is_none()
        );

        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "my_script",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let fetched = scripts_db
            .get_user_script_by_id(user.id, script.id)
            .await?
            .unwrap();
        assert_eq!(fetched.name, "my_script");
        assert_eq!(fetched.content, "console.log('hello');");

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_script(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        let mut script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "my_script",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        script.content = "new content".to_string();
        script.updated_at = OffsetDateTime::from_unix_timestamp(946720900)?;
        scripts_db
            .update_user_script(user.id, &script, None)
            .await?;

        let fetched = scripts_db
            .get_user_script_by_id(user.id, script.id)
            .await?
            .unwrap();
        assert_eq!(fetched.content, "new content");

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_script(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        assert!(
            scripts_db
                .remove_user_script(user.id, uuid!("00000000-0000-0000-0000-000000000099"))
                .await?
                .is_none()
        );

        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "to_delete",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let removed = scripts_db
            .remove_user_script(user.id, script.id)
            .await?
            .unwrap();
        assert_eq!(removed.name, "to_delete");
        assert!(scripts_db.get_user_scripts(user.id).await?.is_empty());
        assert!(
            scripts_db
                .remove_user_script(user.id, script.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn scripts_are_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let scripts_db = db.scripts();
        let script_a = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "shared_name",
            "responder",
        )?;
        scripts_db.insert_user_script(user_a.id, &script_a).await?;

        let mut script_b = mock_script(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "shared_name",
            "responder",
        )?;
        script_b.content = "user_b_content".to_string();
        scripts_db.insert_user_script(user_b.id, &script_b).await?;

        let scripts_a = scripts_db.get_user_scripts(user_a.id).await?;
        let scripts_b = scripts_db.get_user_scripts(user_b.id).await?;
        assert_eq!(scripts_a.len(), 1);
        assert_eq!(scripts_b.len(), 1);
        assert_eq!(scripts_a[0].content, "console.log('hello');");
        assert_eq!(scripts_b[0].content, "user_b_content");

        Ok(())
    }

    #[sqlx::test]
    async fn duplicate_name_returns_conflict_error(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "my_script",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let script2 = mock_script(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "my_script",
            "responder",
        )?;
        let err = scripts_db
            .insert_user_script(user.id, &script2)
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

        let scripts_db = db.scripts();
        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "script1",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;
        assert_eq!(scripts_db.count_user_scripts(user.id).await?, 1);

        db.remove_user_by_email(&user.email).await?;
        assert_eq!(scripts_db.count_user_scripts(user.id).await?, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_scripts_empty(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts = db.scripts().bulk_get_user_scripts(user.id, &[]).await?;
        assert!(scripts.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_scripts_returns_matching(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        let script_a = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "alpha_script",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script_a).await?;

        let script_b = mock_script(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "beta_script",
            "api_extractor",
        )?;
        scripts_db.insert_user_script(user.id, &script_b).await?;

        let script_c = mock_script(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "gamma_script",
            "universal",
        )?;
        scripts_db.insert_user_script(user.id, &script_c).await?;

        let scripts = scripts_db
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

        let scripts_db = db.scripts();
        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "my_script",
            "responder",
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let scripts = scripts_db
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

        let scripts_db = db.scripts();
        let script_a = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "shared_name",
            "responder",
        )?;
        scripts_db.insert_user_script(user_a.id, &script_a).await?;

        let script_b = mock_script(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "shared_name",
            "responder",
        )?;
        scripts_db.insert_user_script(user_b.id, &script_b).await?;

        let scripts = scripts_db
            .bulk_get_user_scripts(user_b.id, &[script_a.id])
            .await?;
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

    // --- Tag tests ---

    #[sqlx::test]
    async fn insert_script_with_tags_is_atomic(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[tag_a.id, tag_b.id],
        )?;
        let tags = scripts_db.insert_user_script(user.id, &script).await?;

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[1].name, "beta");

        let fetched = scripts_db.get_user_script_by_id(user.id, script.id).await?;
        assert!(fetched.is_some());

        let tags_map = scripts_db.get_script_tags(&[script.id]).await?;
        assert_eq!(tags_map[&script.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_script_with_tags_empty_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        let script = mock_script(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "responder",
        )?;
        let tags = scripts_db.insert_user_script(user.id, &script).await?;

        assert!(tags.is_empty());

        let fetched = scripts_db.get_user_script_by_id(user.id, script.id).await?;
        assert!(fetched.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn update_script_with_tags_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[tag_a.id, tag_b.id],
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let mut updated = script.clone();
        updated.content = "updated content".to_string();
        updated.updated_at = OffsetDateTime::from_unix_timestamp(946720900)?;
        let tags = scripts_db
            .update_user_script(user.id, &updated, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let fetched = scripts_db
            .get_user_script_by_id(user.id, script.id)
            .await?
            .unwrap();
        assert_eq!(fetched.content, "updated content");

        Ok(())
    }

    #[sqlx::test]
    async fn update_script_with_tags_clears_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[tag.id],
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let mut updated = script.clone();
        updated.updated_at = OffsetDateTime::from_unix_timestamp(946720900)?;
        let tags = scripts_db
            .update_user_script(user.id, &updated, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = scripts_db.get_script_tags(&[script.id]).await?;
        assert!(!tags_map.contains_key(&script.id) || tags_map[&script.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_script_with_tags_rolls_back_on_invalid_tags(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[uuid!("00000000-0000-0000-0000-000000000099")],
        )?;
        let result = scripts_db.insert_user_script(user.id, &script).await;
        assert!(result.is_err());

        let fetched = scripts_db.get_user_script_by_id(user.id, script.id).await?;
        assert!(fetched.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_script_returns_tags_ordered_by_name(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let tag_z = db.insert_user_tag(user.id, "zebra", "primary").await?;
        let tag_a = db.insert_user_tag(user.id, "alpha", "danger").await?;
        let tag_m = db.insert_user_tag(user.id, "middle", "success").await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[tag_z.id, tag_a.id, tag_m.id],
        )?;
        let tags = scripts_db.insert_user_script(user.id, &script).await?;
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_script_with_tags_handles_duplicate_tag_ids(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[tag.id, tag.id],
        )?;
        let result = scripts_db.insert_user_script(user.id, &script).await;

        match result {
            Ok(tags) => {
                assert_eq!(tags.len(), 1);
                assert_eq!(tags[0].name, "alpha");
            }
            Err(_) => {
                let fetched = scripts_db.get_user_script_by_id(user.id, script.id).await?;
                assert!(fetched.is_none());
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn update_script_with_tags_rolls_back_on_invalid_tags(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let scripts_db = db.scripts();
        let script = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            &[tag.id],
        )?;
        scripts_db.insert_user_script(user.id, &script).await?;

        let mut updated = script.clone();
        updated.content = "updated-content".to_string();
        updated.updated_at = OffsetDateTime::from_unix_timestamp(946720900)?;
        let result = scripts_db
            .update_user_script(
                user.id,
                &updated,
                Some(vec![uuid!("00000000-0000-0000-0000-000000000099")]),
            )
            .await;
        assert!(result.is_err());

        let fetched = scripts_db
            .get_user_script_by_id(user.id, script.id)
            .await?
            .unwrap();
        assert_eq!(fetched.content, "console.log('hello');");

        let tags_map = scripts_db.get_script_tags(&[script.id]).await?;
        assert_eq!(tags_map[&script.id].len(), 1);
        assert_eq!(tags_map[&script.id][0].name, "alpha");

        Ok(())
    }

    #[sqlx::test]
    async fn insert_script_with_tags_isolated_between_users(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let tag_a = db.insert_user_tag(user_a.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user_b.id, "beta", "danger").await?;

        let scripts_db = db.scripts();
        let script_a = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "script-a",
            &[tag_a.id],
        )?;
        let tags_a = scripts_db.insert_user_script(user_a.id, &script_a).await?;

        let script_b = mock_script_with_tags(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "script-b",
            &[tag_b.id],
        )?;
        let tags_b = scripts_db.insert_user_script(user_b.id, &script_b).await?;

        assert_eq!(tags_a.len(), 1);
        assert_eq!(tags_a[0].name, "alpha");
        assert_eq!(tags_b.len(), 1);
        assert_eq!(tags_b[0].name, "beta");

        let map_a = scripts_db.get_script_tags(&[script_a.id]).await?;
        let map_b = scripts_db.get_script_tags(&[script_b.id]).await?;
        assert_eq!(map_a[&script_a.id][0].name, "alpha");
        assert_eq!(map_b[&script_b.id][0].name, "beta");

        Ok(())
    }
}
