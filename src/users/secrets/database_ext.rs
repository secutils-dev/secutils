use crate::{
    database::Database,
    error::Error,
    server::{ListParams, TagJunction, count_sql, list_sql},
    users::{EntityTag, RawEntityTag, UserId, group_entity_tags, secrets::UserSecret},
};
use sqlx::{Acquire, Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

/// Junction table linking secrets to tags.
const SECRETS_TAG_JUNCTION: TagJunction = TagJunction {
    table: "user_data_secrets_tags",
    entity_col: "secret_id",
};

#[derive(Debug, sqlx::FromRow)]
pub(super) struct RawUserSecret {
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
            tags: vec![],
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// A database extension for secret-related operations.
pub struct SecretsDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
}

impl<'pool> SecretsDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>) -> Self {
        Self { pool }
    }

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
        .fetch_all(self.pool)
        .await?;

        Ok(raw
            .into_iter()
            .map(|r| r.into_user_secret(include_values))
            .collect())
    }

    /// Returns a single page of secrets for a user (metadata only, no values), honoring search,
    /// tag, sort, and pagination parameters, together with the total number of matching secrets.
    ///
    /// `sort_col` must originate from the caller's static allowlist.
    pub async fn get_user_secrets_page(
        &self,
        user_id: UserId,
        params: &ListParams,
        sort_col: &str,
    ) -> anyhow::Result<(Vec<UserSecret>, i64)> {
        let list = list_sql(
            "user_data_secrets",
            "id, user_id, name, ''::bytea AS value, created_at, updated_at",
            "name",
            &SECRETS_TAG_JUNCTION,
            sort_col,
            params.order,
        );
        let rows: Vec<RawUserSecret> = sqlx::query_as(&list)
            .bind(*user_id)
            .bind(params.query.as_deref())
            .bind(params.tags.as_slice())
            .bind(params.global_tags.as_slice())
            .bind(params.limit)
            .bind(params.offset)
            .fetch_all(self.pool)
            .await?;

        let count = count_sql("user_data_secrets", "name", &SECRETS_TAG_JUNCTION);
        let total: i64 = sqlx::query_scalar(&count)
            .bind(*user_id)
            .bind(params.query.as_deref())
            .bind(params.tags.as_slice())
            .bind(params.global_tags.as_slice())
            .fetch_one(self.pool)
            .await?;

        Ok((
            rows.into_iter()
                .map(|r| r.into_user_secret(false))
                .collect(),
            total,
        ))
    }

    /// Lists secrets for a user matching the specified IDs (metadata only, no values).
    pub async fn bulk_get_user_secrets(
        &self,
        user_id: UserId,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<UserSecret>> {
        let raw: Vec<RawUserSecret> = query_as!(
            RawUserSecret,
            r#"
SELECT id, user_id, name, value, created_at, updated_at
FROM user_data_secrets
WHERE user_id = $1 AND id = ANY($2)
ORDER BY name ASC
            "#,
            *user_id,
            ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(raw.into_iter().map(|r| r.into_user_secret(false)).collect())
    }

    /// Counts secrets for a user.
    pub async fn count_user_secrets(&self, user_id: UserId) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM user_data_secrets WHERE user_id = $1"#,
            *user_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Inserts a new secret (and associated tags). Returns resolved tags.
    pub async fn insert_user_secret(
        &self,
        user_id: UserId,
        secret: &UserSecret,
    ) -> anyhow::Result<Vec<EntityTag>> {
        let encrypted_value = secret.encrypted_value.as_deref().unwrap_or_default();
        let mut tx = self.pool.begin().await?;
        match query!(
            r#"
INSERT INTO user_data_secrets (id, user_id, name, value, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            secret.id,
            *user_id,
            secret.name,
            encrypted_value,
            secret.created_at,
            secret.updated_at
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
                        "A secret with name '{}' already exists.",
                        secret.name
                    )))
                } else {
                    err.into()
                });
            }
        }

        let tags = if secret.tags.is_empty() {
            vec![]
        } else {
            Self::set_secret_tags(
                &mut *tx,
                secret.id,
                &secret.tags.iter().map(|t| t.id).collect::<Vec<_>>(),
            )
            .await?
        };

        tx.commit().await?;
        Ok(tags)
    }

    /// Updates the encrypted value of an existing secret (and optionally associated tags).
    /// When `tag_ids` is `Some`, tags are replaced; when `None`, tags are left unchanged.
    pub async fn update_user_secret(
        &self,
        user_id: UserId,
        secret: &UserSecret,
        tag_ids: Option<Vec<Uuid>>,
    ) -> anyhow::Result<Option<Vec<EntityTag>>> {
        let encrypted_value = secret.encrypted_value.as_deref().unwrap_or_default();
        let mut tx = self.pool.begin().await?;
        let result = query!(
            r#"
UPDATE user_data_secrets SET value = $1, updated_at = $2
WHERE user_id = $3 AND id = $4
            "#,
            encrypted_value,
            secret.updated_at,
            *user_id,
            secret.id
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::Error::from(Error::not_found(format!(
                "Secret '{}' not found.",
                secret.id
            ))));
        }

        let updated_tags = if let Some(ref tag_ids) = tag_ids {
            Some(Self::set_secret_tags(&mut *tx, secret.id, tag_ids).await?)
        } else {
            None
        };

        tx.commit().await?;
        Ok(updated_tags)
    }

    /// Removes a secret by user_id and id.
    pub async fn remove_user_secret(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<UserSecret>> {
        let raw: Option<RawUserSecret> = query_as!(
            RawUserSecret,
            r#"
DELETE FROM user_data_secrets
WHERE user_id = $1 AND id = $2
RETURNING id as "id!", user_id as "user_id!", name as "name!", value as "value!", created_at as "created_at!", updated_at as "updated_at!"
            "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(raw.map(|r| r.into_user_secret(false)))
    }

    /// Fetches tags for a batch of secrets.
    pub async fn get_secret_tags(
        &self,
        entity_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, Vec<EntityTag>>> {
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = query_as!(
            RawEntityTag,
            r#"
SELECT jt.secret_id AS entity_id, t.id, t.name, t.color
FROM user_data_secrets_tags jt
JOIN user_tags t ON jt.tag_id = t.id
WHERE jt.secret_id = ANY($1)
ORDER BY t.name ASC
            "#,
            entity_ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(group_entity_tags(rows))
    }

    async fn set_secret_tags<'a>(
        executor: impl Acquire<'a, Database = Postgres>,
        entity_id: Uuid,
        tag_ids: &[Uuid],
    ) -> anyhow::Result<Vec<EntityTag>> {
        let mut conn = executor.acquire().await?;
        query!(
            "DELETE FROM user_data_secrets_tags WHERE secret_id = $1",
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
    INSERT INTO user_data_secrets_tags (secret_id, tag_id)
    SELECT $1, unnest($2::uuid[])
    RETURNING secret_id, tag_id
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
    /// Returns a database extension for secret-related operations.
    pub fn secrets(&self) -> SecretsDatabaseExt<'_> {
        SecretsDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error,
        tests::{mock_user, mock_user_with_id},
        users::{EntityTag, secrets::UserSecret},
    };
    use actix_web::{ResponseError, http::StatusCode};
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::{Uuid, uuid};

    fn mock_secret(id: Uuid, name: &str) -> UserSecret {
        let now = OffsetDateTime::now_utc();
        UserSecret {
            id,
            user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            name: name.to_string(),
            encrypted_value: Some(b"encrypted-value".to_vec()),
            tags: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    fn mock_secret_with_tags(id: Uuid, name: &str, tag_ids: &[Uuid]) -> UserSecret {
        let mut secret = mock_secret(id, name);
        secret.tags = tag_ids.iter().map(|id| EntityTag::from(*id)).collect();
        secret
    }

    #[sqlx::test]
    async fn can_insert_and_list_secrets(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        assert!(
            secrets_db
                .get_user_secrets(user.id, false)
                .await?
                .is_empty()
        );
        assert_eq!(secrets_db.count_user_secrets(user.id).await?, 0);

        let secret = mock_secret(Uuid::now_v7(), "API_KEY");
        secrets_db.insert_user_secret(user.id, &secret).await?;

        let secret_b = mock_secret(Uuid::now_v7(), "DB_PASSWORD");
        secrets_db.insert_user_secret(user.id, &secret_b).await?;

        let secrets = secrets_db.get_user_secrets(user.id, false).await?;
        assert_eq!(secrets.len(), 2);
        assert_eq!(secrets[0].name, "API_KEY");
        assert!(secrets[0].encrypted_value.is_none());
        assert_eq!(secrets[1].name, "DB_PASSWORD");
        assert_eq!(secrets_db.count_user_secrets(user.id).await?, 2);

        Ok(())
    }

    fn page_params(
        page: u32,
        size: u32,
        order: crate::server::SortOrder,
        q: Option<&str>,
    ) -> crate::server::ListParams {
        crate::server::PaginationParams {
            page: Some(page),
            page_size: Some(size),
            order: Some(order),
            q: q.map(str::to_string),
            ..Default::default()
        }
        .resolve()
    }

    #[sqlx::test]
    async fn paginates_secrets_by_name(pool: PgPool) -> anyhow::Result<()> {
        use crate::server::SortOrder;
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        for i in 0..25 {
            secrets_db
                .insert_user_secret(
                    user.id,
                    &mock_secret(Uuid::now_v7(), &format!("SECRET_{i:02}")),
                )
                .await?;
        }

        // First page.
        let (items, total) = secrets_db
            .get_user_secrets_page(user.id, &page_params(0, 10, SortOrder::Asc, None), "name")
            .await?;
        assert_eq!(total, 25);
        assert_eq!(items.len(), 10);
        assert_eq!(items[0].name, "SECRET_00");
        assert_eq!(items[9].name, "SECRET_09");

        // Last (partial) page.
        let (items, total) = secrets_db
            .get_user_secrets_page(user.id, &page_params(2, 10, SortOrder::Asc, None), "name")
            .await?;
        assert_eq!(total, 25);
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].name, "SECRET_20");

        // Descending order.
        let (items, _) = secrets_db
            .get_user_secrets_page(user.id, &page_params(0, 3, SortOrder::Desc, None), "name")
            .await?;
        assert_eq!(items[0].name, "SECRET_24");
        assert!(items[0].encrypted_value.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn paginated_secrets_filter_by_search(pool: PgPool) -> anyhow::Result<()> {
        use crate::server::SortOrder;
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        for name in ["ALPHA_KEY", "ALPHA_TOKEN", "BETA_KEY"] {
            secrets_db
                .insert_user_secret(user.id, &mock_secret(Uuid::now_v7(), name))
                .await?;
        }

        let (items, total) = secrets_db
            .get_user_secrets_page(
                user.id,
                &page_params(0, 10, SortOrder::Asc, Some("alpha")),
                "name",
            )
            .await?;
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "ALPHA_KEY");
        assert_eq!(items[1].name, "ALPHA_TOKEN");

        // Wildcard characters in the query are matched literally (no injection).
        let (_, total) = secrets_db
            .get_user_secrets_page(
                user.id,
                &page_params(0, 10, SortOrder::Asc, Some("%")),
                "name",
            )
            .await?;
        assert_eq!(total, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn paginated_secrets_filter_by_tags(pool: PgPool) -> anyhow::Result<()> {
        use crate::server::{ListParams, SortOrder};
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = create_tag(&db, user.id, "alpha").await;
        let tag_b = create_tag(&db, user.id, "beta").await;

        let secrets_db = db.secrets();
        secrets_db
            .insert_user_secret(
                user.id,
                &mock_secret_with_tags(Uuid::now_v7(), "ONLY_A", &[tag_a]),
            )
            .await?;
        secrets_db
            .insert_user_secret(
                user.id,
                &mock_secret_with_tags(Uuid::now_v7(), "ONLY_B", &[tag_b]),
            )
            .await?;
        secrets_db
            .insert_user_secret(
                user.id,
                &mock_secret_with_tags(Uuid::now_v7(), "BOTH_AB", &[tag_a, tag_b]),
            )
            .await?;
        secrets_db
            .insert_user_secret(user.id, &mock_secret(Uuid::now_v7(), "NO_TAGS"))
            .await?;

        let base = |tags: Vec<Uuid>, global: Vec<Uuid>| ListParams {
            offset: 0,
            limit: 50,
            order: SortOrder::Asc,
            query: None,
            tags,
            global_tags: global,
        };

        // Page-level OR: ANY of [a, b].
        let (items, total) = secrets_db
            .get_user_secrets_page(user.id, &base(vec![tag_a, tag_b], vec![]), "name")
            .await?;
        assert_eq!(total, 3);
        let names: Vec<_> = items.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["BOTH_AB", "ONLY_A", "ONLY_B"]);

        // Global AND: must have BOTH a and b.
        let (items, total) = secrets_db
            .get_user_secrets_page(user.id, &base(vec![], vec![tag_a, tag_b]), "name")
            .await?;
        assert_eq!(total, 1);
        assert_eq!(items[0].name, "BOTH_AB");

        // Combined OR + AND.
        let (items, total) = secrets_db
            .get_user_secrets_page(user.id, &base(vec![tag_a], vec![tag_b]), "name")
            .await?;
        assert_eq!(total, 1);
        assert_eq!(items[0].name, "BOTH_AB");

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_secrets_with_values(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let mut secret_a = mock_secret(Uuid::now_v7(), "KEY_A");
        secret_a.encrypted_value = Some(b"val-a".to_vec());
        secrets_db.insert_user_secret(user.id, &secret_a).await?;

        let mut secret_b = mock_secret(Uuid::now_v7(), "KEY_B");
        secret_b.encrypted_value = Some(b"val-b".to_vec());
        secrets_db.insert_user_secret(user.id, &secret_b).await?;

        let secrets = secrets_db.get_user_secrets(user.id, true).await?;
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

        let secrets_db = db.secrets();
        let mut original = mock_secret(Uuid::now_v7(), "API_KEY");
        original.encrypted_value = Some(b"old-val".to_vec());
        secrets_db.insert_user_secret(user.id, &original).await?;

        let mut updated = original.clone();
        updated.encrypted_value = Some(b"new-val".to_vec());
        updated.updated_at = OffsetDateTime::now_utc();
        secrets_db
            .update_user_secret(user.id, &updated, None)
            .await?;

        let secrets = secrets_db.get_user_secrets(user.id, true).await?;
        let secret = secrets.iter().find(|s| s.name == "API_KEY").unwrap();
        assert_eq!(
            secret.encrypted_value.as_deref(),
            Some(b"new-val".as_slice())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_not_found(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secret = mock_secret(Uuid::now_v7(), "API_KEY");
        let err = db
            .secrets()
            .update_user_secret(user.id, &secret, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_secret(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        assert!(
            secrets_db
                .remove_user_secret(user.id, uuid!("00000000-0000-0000-0000-000000000099"))
                .await?
                .is_none()
        );

        let secret = mock_secret(Uuid::now_v7(), "API_KEY");
        secrets_db.insert_user_secret(user.id, &secret).await?;

        let removed = secrets_db
            .remove_user_secret(user.id, secret.id)
            .await?
            .unwrap();
        assert_eq!(removed.name, "API_KEY");
        assert!(
            secrets_db
                .get_user_secrets(user.id, false)
                .await?
                .is_empty()
        );
        assert!(
            secrets_db
                .remove_user_secret(user.id, secret.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn secrets_are_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let secrets_db = db.secrets();
        let mut secret_a = mock_secret(Uuid::now_v7(), "SHARED_NAME");
        secret_a.encrypted_value = Some(b"val-a".to_vec());
        secrets_db.insert_user_secret(user_a.id, &secret_a).await?;

        let mut secret_b = mock_secret(Uuid::now_v7(), "SHARED_NAME");
        secret_b.encrypted_value = Some(b"val-b".to_vec());
        secrets_db.insert_user_secret(user_b.id, &secret_b).await?;

        let secrets_a = secrets_db.get_user_secrets(user_a.id, true).await?;
        let secrets_b = secrets_db.get_user_secrets(user_b.id, true).await?;
        assert_eq!(
            secrets_a[0].encrypted_value.as_deref(),
            Some(b"val-a".as_slice())
        );
        assert_eq!(
            secrets_b[0].encrypted_value.as_deref(),
            Some(b"val-b".as_slice())
        );

        assert_eq!(secrets_db.count_user_secrets(user_a.id).await?, 1);
        assert_eq!(secrets_db.count_user_secrets(user_b.id).await?, 1);

        Ok(())
    }

    #[sqlx::test]
    async fn duplicate_name_returns_conflict_error(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let secret_a = mock_secret(Uuid::now_v7(), "API_KEY");
        secrets_db.insert_user_secret(user.id, &secret_a).await?;

        let secret_b = mock_secret(Uuid::now_v7(), "API_KEY");
        let err = secrets_db
            .insert_user_secret(user.id, &secret_b)
            .await
            .unwrap_err();

        let typed = err.downcast::<Error>().unwrap();
        assert_eq!(typed.status_code(), StatusCode::CONFLICT);
        assert!(typed.root_cause.to_string().contains("API_KEY"));

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_secrets_empty(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets = db.secrets().bulk_get_user_secrets(user.id, &[]).await?;
        assert!(secrets.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_secrets_returns_matching(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let secret_a = mock_secret(Uuid::now_v7(), "API_KEY");
        secrets_db.insert_user_secret(user.id, &secret_a).await?;

        let secret_b = mock_secret(Uuid::now_v7(), "DB_PASSWORD");
        secrets_db.insert_user_secret(user.id, &secret_b).await?;

        let secret_c = mock_secret(Uuid::now_v7(), "TOKEN");
        secrets_db.insert_user_secret(user.id, &secret_c).await?;

        let secrets = secrets_db
            .bulk_get_user_secrets(user.id, &[secret_a.id, secret_b.id])
            .await?;
        assert_eq!(secrets.len(), 2);
        assert_eq!(secrets[0].name, "API_KEY");
        assert_eq!(secrets[1].name, "DB_PASSWORD");
        assert!(secrets[0].encrypted_value.is_none());
        assert!(secrets[1].encrypted_value.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_secrets_ignores_non_existent(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let secret = mock_secret(Uuid::now_v7(), "API_KEY");
        secrets_db.insert_user_secret(user.id, &secret).await?;

        let secrets = secrets_db
            .bulk_get_user_secrets(
                user.id,
                &[secret.id, uuid!("00000000-0000-0000-0000-000000000099")],
            )
            .await?;
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].name, "API_KEY");

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_secrets_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let secrets_db = db.secrets();
        let secret_a = mock_secret(Uuid::now_v7(), "SHARED_NAME");
        secrets_db.insert_user_secret(user_a.id, &secret_a).await?;

        let secret_b = mock_secret(Uuid::now_v7(), "SHARED_NAME");
        secrets_db.insert_user_secret(user_b.id, &secret_b).await?;

        let secrets = secrets_db
            .bulk_get_user_secrets(user_b.id, &[secret_a.id])
            .await?;
        assert!(secrets.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn cascade_delete_on_user_removal(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let secret = mock_secret(Uuid::now_v7(), "API_KEY");
        secrets_db.insert_user_secret(user.id, &secret).await?;
        assert_eq!(secrets_db.count_user_secrets(user.id).await?, 1);

        db.remove_user_by_email(&user.email).await?;
        assert_eq!(secrets_db.count_user_secrets(user.id).await?, 0);

        Ok(())
    }

    // --- Tag tests ---

    async fn create_tag(db: &Database, user_id: crate::users::UserId, name: &str) -> Uuid {
        db.insert_user_tag(user_id, name, "default")
            .await
            .unwrap()
            .id
    }

    #[sqlx::test]
    async fn insert_secret_with_tags_is_atomic(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = create_tag(&db, user.id, "alpha").await;
        let tag_b = create_tag(&db, user.id, "beta").await;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[tag_a, tag_b]);
        let tags = secrets_db.insert_user_secret(user.id, &secret).await?;
        assert_eq!(tags.len(), 2);

        let mut tags_map = secrets_db.get_secret_tags(&[secret.id]).await?;
        let fetched = tags_map.remove(&secret.id).unwrap();
        assert_eq!(fetched.len(), 2);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_secret_with_tags_empty_tags(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let secret = mock_secret(Uuid::now_v7(), "API_KEY");
        let tags = secrets_db.insert_user_secret(user.id, &secret).await?;
        assert!(tags.is_empty());

        let secrets = secrets_db.get_user_secrets(user.id, false).await?;
        assert_eq!(secrets.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_with_tags_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = create_tag(&db, user.id, "alpha").await;
        let tag_b = create_tag(&db, user.id, "beta").await;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[tag_a]);
        secrets_db.insert_user_secret(user.id, &secret).await?;

        let mut updated = secret.clone();
        updated.updated_at = OffsetDateTime::now_utc();
        let tags = secrets_db
            .update_user_secret(user.id, &updated, Some(vec![tag_b]))
            .await?
            .unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "beta");

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_with_tags_clears_tags(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = create_tag(&db, user.id, "alpha").await;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[tag_a]);
        secrets_db.insert_user_secret(user.id, &secret).await?;

        let mut updated = secret.clone();
        updated.updated_at = OffsetDateTime::now_utc();
        let tags = secrets_db
            .update_user_secret(user.id, &updated, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_secret_with_tags_rolls_back_on_invalid_tags(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[Uuid::now_v7()]);
        let result = secrets_db.insert_user_secret(user.id, &secret).await;
        assert!(result.is_err());

        // Entity should not exist after rollback.
        let secrets = secrets_db.get_user_secrets(user.id, false).await?;
        assert!(secrets.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_secret_returns_tags_ordered_by_name(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_z = create_tag(&db, user.id, "zulu").await;
        let tag_a = create_tag(&db, user.id, "alpha").await;
        let tag_m = create_tag(&db, user.id, "mike").await;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[tag_z, tag_a, tag_m]);
        let tags = secrets_db.insert_user_secret(user.id, &secret).await?;
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[1].name, "mike");
        assert_eq!(tags[2].name, "zulu");

        Ok(())
    }

    #[sqlx::test]
    async fn insert_secret_with_tags_handles_duplicate_tag_ids(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = create_tag(&db, user.id, "alpha").await;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[tag_a, tag_a]);
        // Duplicate tag IDs in insert: should either succeed or fail, but not panic.
        let _result = secrets_db.insert_user_secret(user.id, &secret).await;

        Ok(())
    }

    #[sqlx::test]
    async fn update_secret_with_tags_rolls_back_on_invalid_tags(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let tag_a = create_tag(&db, user.id, "alpha").await;

        let secrets_db = db.secrets();
        let secret = mock_secret_with_tags(Uuid::now_v7(), "API_KEY", &[tag_a]);
        secrets_db.insert_user_secret(user.id, &secret).await?;

        let mut updated = secret.clone();
        updated.updated_at = OffsetDateTime::now_utc();
        let result = secrets_db
            .update_user_secret(user.id, &updated, Some(vec![Uuid::now_v7()]))
            .await;
        assert!(result.is_err());

        // Original tags should still be intact.
        let mut tags_map = secrets_db.get_secret_tags(&[secret.id]).await?;
        let fetched = tags_map.remove(&secret.id).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].name, "alpha");

        Ok(())
    }

    #[sqlx::test]
    async fn insert_secret_with_tags_isolated_between_users(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        db.upsert_user(&user_a).await?;
        db.upsert_user(&user_b).await?;

        let tag_a = create_tag(&db, user_a.id, "alpha").await;
        let tag_b = create_tag(&db, user_b.id, "beta").await;

        let secrets_db = db.secrets();
        let secret_a = mock_secret_with_tags(Uuid::now_v7(), "KEY_A", &[tag_a]);
        secrets_db.insert_user_secret(user_a.id, &secret_a).await?;

        let secret_b = mock_secret_with_tags(Uuid::now_v7(), "KEY_B", &[tag_b]);
        secrets_db.insert_user_secret(user_b.id, &secret_b).await?;

        let mut tags_map_a = secrets_db.get_secret_tags(&[secret_a.id]).await?;
        let mut tags_map_b = secrets_db.get_secret_tags(&[secret_b.id]).await?;
        let tags_a = tags_map_a.remove(&secret_a.id).unwrap();
        let tags_b = tags_map_b.remove(&secret_b.id).unwrap();
        assert_eq!(tags_a.len(), 1);
        assert_eq!(tags_a[0].name, "alpha");
        assert_eq!(tags_b.len(), 1);
        assert_eq!(tags_b[0].name, "beta");

        Ok(())
    }
}
