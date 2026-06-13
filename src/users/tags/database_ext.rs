use crate::{
    database::Database,
    error::Error,
    server::ListParams,
    users::{UserId, tags::UserTag},
};
use sqlx::{Row, error::ErrorKind as SqlxErrorKind};
use time::OffsetDateTime;
use uuid::Uuid;

fn row_to_user_tag(row: &sqlx::postgres::PgRow) -> UserTag {
    UserTag {
        id: row.get("id"),
        user_id: row.get::<Uuid, _>("user_id").into(),
        name: row.get("name"),
        color: row.get("color"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

impl Database {
    /// Lists all tags for a user, ordered by name.
    pub async fn get_user_tags(&self, user_id: UserId) -> anyhow::Result<Vec<UserTag>> {
        let rows = sqlx::query(
            r#"SELECT id, user_id, name, color, created_at, updated_at
               FROM user_tags
               WHERE user_id = $1
               ORDER BY name ASC"#,
        )
        .bind(*user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_user_tag).collect())
    }

    /// Returns a single page of tags for a user, honoring search, sort, and pagination parameters,
    /// plus the total count. Tags have no sub-tags, so no tag filtering is applied.
    ///
    /// `sort_col` MUST originate from the caller's static allowlist.
    pub async fn get_user_tags_page(
        &self,
        user_id: UserId,
        params: &ListParams,
        sort_col: &str,
    ) -> anyhow::Result<(Vec<UserTag>, i64)> {
        let ord = params.order.as_sql();
        // The query matches the tag name (case-insensitively) or an exact id, mirroring the shared
        // pagination filter so "filter to a single entity by id" links work uniformly.
        // `user_tags.name` uses a nondeterministic (case-insensitive) collation, which Postgres
        // rejects for `ILIKE`, force a deterministic collation for the match.
        let list = format!(
            "SELECT id, user_id, name, color, created_at, updated_at FROM user_tags \
             WHERE user_id = $1 \
             AND ($2::text IS NULL OR name COLLATE \"C\" ILIKE ('%' || $2 || '%') ESCAPE '\\' OR id::text = $2) \
             ORDER BY {sort_col} {ord}, id {ord} LIMIT $3 OFFSET $4"
        );
        let rows = sqlx::query(&list)
            .bind(*user_id)
            .bind(params.query.as_deref())
            .bind(params.limit)
            .bind(params.offset)
            .fetch_all(&self.pool)
            .await?;

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_tags \
             WHERE user_id = $1 \
             AND ($2::text IS NULL OR name COLLATE \"C\" ILIKE ('%' || $2 || '%') ESCAPE '\\' OR id::text = $2)",
        )
        .bind(*user_id)
        .bind(params.query.as_deref())
        .fetch_one(&self.pool)
        .await?;

        Ok((rows.iter().map(row_to_user_tag).collect(), total))
    }

    /// Fetches tags for a user filtered by a list of IDs.
    pub async fn bulk_get_user_tags(
        &self,
        user_id: UserId,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<UserTag>> {
        let rows = sqlx::query(
            r#"SELECT id, user_id, name, color, created_at, updated_at
               FROM user_tags
               WHERE user_id = $1 AND id = ANY($2)
               ORDER BY name ASC"#,
        )
        .bind(*user_id)
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_user_tag).collect())
    }

    /// Counts tags for a user.
    pub async fn count_user_tags(&self, user_id: UserId) -> anyhow::Result<i64> {
        let row = sqlx::query(r#"SELECT COUNT(*) as count FROM user_tags WHERE user_id = $1"#)
            .bind(*user_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }

    /// Inserts a new tag. Returns the created tag.
    pub async fn insert_user_tag(
        &self,
        user_id: UserId,
        name: &str,
        color: &str,
    ) -> anyhow::Result<UserTag> {
        let id = Uuid::now_v7();
        let now = OffsetDateTime::now_utc();
        match sqlx::query(
            r#"INSERT INTO user_tags (id, user_id, name, color, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(*user_id)
        .bind(name)
        .bind(color)
        .bind(now)
        .bind(now)
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
                        "A tag with name '{name}' already exists."
                    )))
                } else {
                    err.into()
                });
            }
        }

        Ok(UserTag {
            id,
            user_id,
            name: name.to_string(),
            color: color.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Updates an existing tag's name and/or color. Returns the updated tag, or `None` if not
    /// found.
    pub async fn update_user_tag(
        &self,
        user_id: UserId,
        id: Uuid,
        name: Option<&str>,
        color: Option<&str>,
    ) -> anyhow::Result<Option<UserTag>> {
        let now = OffsetDateTime::now_utc();
        let result = sqlx::query(
            r#"UPDATE user_tags
               SET name = COALESCE($1, name),
                   color = COALESCE($2, color),
                   updated_at = $3
               WHERE user_id = $4 AND id = $5
               RETURNING id, user_id, name, color, created_at, updated_at"#,
        )
        .bind(name)
        .bind(color)
        .bind(now)
        .bind(*user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(row) => Ok(row.as_ref().map(row_to_user_tag)),
            Err(err) => {
                let is_conflict = err
                    .as_database_error()
                    .map(|db_err| matches!(db_err.kind(), SqlxErrorKind::UniqueViolation))
                    .unwrap_or_default();
                if is_conflict {
                    Err(anyhow::Error::from(Error::conflict(format!(
                        "A tag with name '{}' already exists.",
                        name.unwrap_or_default()
                    ))))
                } else {
                    Err(err.into())
                }
            }
        }
    }

    /// Removes a tag by user_id and id. Returns the removed tag, or `None` if not found.
    /// Junction table rows are cascade-deleted automatically.
    pub async fn remove_user_tag(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<UserTag>> {
        let row = sqlx::query(
            r#"DELETE FROM user_tags
               WHERE user_id = $1 AND id = $2
               RETURNING id, user_id, name, color, created_at, updated_at"#,
        )
        .bind(*user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_user_tag))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error,
        tests::{mock_user, mock_user_with_id},
    };
    use actix_web::{ResponseError, http::StatusCode};
    use sqlx::PgPool;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_insert_and_list_tags(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        assert!(db.get_user_tags(user.id).await?.is_empty());
        assert_eq!(db.count_user_tags(user.id).await?, 0);

        let tag = db.insert_user_tag(user.id, "production", "primary").await?;
        assert_eq!(tag.name, "production");
        assert_eq!(tag.color, "primary");
        assert_eq!(tag.user_id, user.id);

        db.insert_user_tag(user.id, "staging", "warning").await?;

        let tags = db.get_user_tags(user.id).await?;
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "production");
        assert_eq!(tags[1].name, "staging");
        assert_eq!(db.count_user_tags(user.id).await?, 2);

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_tag(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "old-name", "default").await?;

        let updated = db
            .update_user_tag(user.id, tag.id, Some("new-name"), Some("danger"))
            .await?
            .unwrap();
        assert_eq!(updated.name, "new-name");
        assert_eq!(updated.color, "danger");
        assert!(updated.updated_at >= tag.updated_at);

        Ok(())
    }

    #[sqlx::test]
    async fn update_tag_partial_fields(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "my-tag", "primary").await?;

        let updated = db
            .update_user_tag(user.id, tag.id, None, Some("success"))
            .await?
            .unwrap();
        assert_eq!(updated.name, "my-tag");
        assert_eq!(updated.color, "success");

        let updated = db
            .update_user_tag(user.id, updated.id, Some("renamed"), None)
            .await?
            .unwrap();
        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.color, "success");

        Ok(())
    }

    #[sqlx::test]
    async fn update_nonexistent_tag_returns_none(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let result = db
            .update_user_tag(
                user.id,
                uuid!("00000000-0000-0000-0000-000000000099"),
                Some("x"),
                None,
            )
            .await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_tag(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "to-delete", "default").await?;
        assert_eq!(db.count_user_tags(user.id).await?, 1);

        let removed = db.remove_user_tag(user.id, tag.id).await?.unwrap();
        assert_eq!(removed.name, "to-delete");
        assert!(db.get_user_tags(user.id).await?.is_empty());
        assert!(db.remove_user_tag(user.id, tag.id).await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn tags_are_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        db.insert_user_tag(user_a.id, "shared-name", "primary")
            .await?;
        db.insert_user_tag(user_b.id, "shared-name", "danger")
            .await?;

        let tags_a = db.get_user_tags(user_a.id).await?;
        let tags_b = db.get_user_tags(user_b.id).await?;
        assert_eq!(tags_a[0].color, "primary");
        assert_eq!(tags_b[0].color, "danger");
        assert_eq!(db.count_user_tags(user_a.id).await?, 1);
        assert_eq!(db.count_user_tags(user_b.id).await?, 1);

        Ok(())
    }

    #[sqlx::test]
    async fn duplicate_name_returns_conflict_error(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_tag(user.id, "production", "primary").await?;
        let err = db
            .insert_user_tag(user.id, "production", "danger")
            .await
            .unwrap_err();

        let typed = err.downcast::<Error>().unwrap();
        assert_eq!(typed.status_code(), StatusCode::CONFLICT);
        assert!(typed.root_cause.to_string().contains("production"));

        Ok(())
    }

    #[sqlx::test]
    async fn rename_to_existing_name_returns_conflict(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        db.insert_user_tag(user.id, "beta", "danger").await?;

        let err = db
            .update_user_tag(user.id, tag_a.id, Some("beta"), None)
            .await
            .unwrap_err();
        let typed = err.downcast::<Error>().unwrap();
        assert_eq!(typed.status_code(), StatusCode::CONFLICT);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_returns_matching_tags(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        db.insert_user_tag(user.id, "gamma", "default").await?;

        let result = db
            .bulk_get_user_tags(user.id, &[tag_a.id, tag_b.id])
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "alpha");
        assert_eq!(result[1].name, "beta");

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_empty_ids_returns_empty(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_tag(user.id, "alpha", "primary").await?;

        let result = db.bulk_get_user_tags(user.id, &[]).await?;
        assert!(result.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_ignores_nonexistent_ids(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let fake_id = uuid!("00000000-0000-0000-0000-000000000099");

        let result = db.bulk_get_user_tags(user.id, &[tag.id, fake_id]).await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "alpha");

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let tag_a = db.insert_user_tag(user_a.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user_b.id, "alpha", "danger").await?;

        // User A should not see user B's tag.
        let result = db
            .bulk_get_user_tags(user_a.id, &[tag_a.id, tag_b.id])
            .await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].color, "primary");

        Ok(())
    }

    #[sqlx::test]
    async fn cascade_delete_on_user_removal(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.insert_user_tag(user.id, "production", "primary").await?;
        assert_eq!(db.count_user_tags(user.id).await?, 1);

        db.remove_user_by_email(&user.email).await?;
        assert_eq!(db.count_user_tags(user.id).await?, 0);

        Ok(())
    }
}
