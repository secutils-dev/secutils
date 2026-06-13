mod raw_content_security_policy;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    server::{ListParams, TagJunction, count_sql, list_sql},
    users::{EntityTag, RawEntityTag, UserId, group_entity_tags},
    utils::web_security::{
        ContentSecurityPolicy, database_ext::raw_content_security_policy::RawContentSecurityPolicy,
    },
};
use anyhow::{anyhow, bail};
use sqlx::{Acquire, Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use std::collections::HashMap;
use uuid::Uuid;

/// Junction table linking content security policies to tags.
const CSP_TAG_JUNCTION: TagJunction = TagJunction {
    table: "user_data_web_security_csp_tags",
    entity_col: "csp_id",
};

/// A database extension for the web security utility-related operations.
pub struct WebSecurityDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
}

impl<'pool> WebSecurityDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Retrieves content security policy for the specified user with the specified ID.
    pub async fn get_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<ContentSecurityPolicy>> {
        query_as!(
            RawContentSecurityPolicy,
            r#"
SELECT id, name, directives, created_at, updated_at
FROM user_data_web_security_csp
WHERE user_id = $1 AND id = $2
                "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(ContentSecurityPolicy::try_from)
        .transpose()
    }

    /// Inserts content security policy (and associated tags). Returns resolved tags.
    pub async fn insert_content_security_policy(
        &self,
        user_id: UserId,
        policy: &ContentSecurityPolicy,
    ) -> anyhow::Result<Vec<EntityTag>> {
        let raw_policy = RawContentSecurityPolicy::try_from(policy)?;
        let mut tx = self.pool.begin().await?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_security_csp (user_id, id, name, directives, created_at, updated_at)
    VALUES ( $1, $2, $3, $4, $5, $6 )
            "#,
            *user_id,
            raw_policy.id,
            raw_policy.name,
            raw_policy.directives,
            raw_policy.created_at,
            raw_policy.updated_at
        )
        .execute(&mut *tx)
        .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::conflict(format!(
                    "Content security policy ('{}') already exists.",
                    policy.name
                ))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create content security policy ('{}') due to unknown reason.",
                    policy.name
                )))
            });
        }

        let tags = if policy.tags.is_empty() {
            vec![]
        } else {
            Self::set_csp_tags(
                &mut *tx,
                policy.id,
                &policy.tags.iter().map(|t| t.id).collect::<Vec<_>>(),
            )
            .await?
        };

        tx.commit().await?;
        Ok(tags)
    }

    /// Updates content security policy (and optionally associated tags). Returns resolved tags
    /// only if `tag_ids` is `Some`.
    pub async fn update_content_security_policy(
        &self,
        user_id: UserId,
        policy: &ContentSecurityPolicy,
        tag_ids: Option<Vec<Uuid>>,
    ) -> anyhow::Result<Option<Vec<EntityTag>>> {
        let raw_policy = RawContentSecurityPolicy::try_from(policy)?;
        let mut tx = self.pool.begin().await?;
        let result = query!(
            r#"
    UPDATE user_data_web_security_csp
    SET name = $3, directives = $4, updated_at = $5
    WHERE user_id = $1 AND id = $2
            "#,
            *user_id,
            raw_policy.id,
            raw_policy.name,
            raw_policy.directives,
            raw_policy.updated_at
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "A content security policy ('{}') doesn't exist.",
                        policy.name
                    )));
                }
            }
            Err(err) => {
                let is_conflict_error = err
                    .as_database_error()
                    .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                    .unwrap_or_default();
                bail!(if is_conflict_error {
                    SecutilsError::conflict(format!(
                        "Content security policy ('{}') already exists.",
                        policy.name
                    ))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update content security policy ('{}') due to unknown reason.",
                        policy.name
                    )))
                });
            }
        }

        let updated_tags = if let Some(ref tag_ids) = tag_ids {
            Some(Self::set_csp_tags(&mut *tx, policy.id, tag_ids).await?)
        } else {
            None
        };

        tx.commit().await?;
        Ok(updated_tags)
    }

    /// Removes content security policy for the specified user with the specified ID.
    pub async fn remove_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<()> {
        query!(
            r#"
    DELETE FROM user_data_web_security_csp
    WHERE user_id = $1 AND id = $2
                    "#,
            *user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves content security policies for the specified user matching the given IDs.
    pub async fn bulk_get_content_security_policies(
        &self,
        user_id: UserId,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<ContentSecurityPolicy>> {
        let raw_policies = query_as!(
            RawContentSecurityPolicy,
            r#"
    SELECT id, name, directives, created_at, updated_at
    FROM user_data_web_security_csp
    WHERE user_id = $1 AND id = ANY($2)
    ORDER BY updated_at
                    "#,
            *user_id,
            ids
        )
        .fetch_all(self.pool)
        .await?;

        let mut policies = vec![];
        for raw_policy in raw_policies {
            policies.push(ContentSecurityPolicy::try_from(raw_policy)?);
        }

        Ok(policies)
    }

    /// Retrieves all content security policies for the specified user.
    pub async fn get_content_security_policies(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<ContentSecurityPolicy>> {
        let raw_policies = query_as!(
            RawContentSecurityPolicy,
            r#"
    SELECT id, name, directives, created_at, updated_at
    FROM user_data_web_security_csp
    WHERE user_id = $1
    ORDER BY updated_at
                    "#,
            *user_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut policies = vec![];
        for raw_policy in raw_policies {
            policies.push(ContentSecurityPolicy::try_from(raw_policy)?);
        }

        Ok(policies)
    }

    /// Returns a single page of content security policies for a user, honoring search, tag, sort,
    /// and pagination parameters, plus the total count.
    ///
    /// `sort_col` MUST originate from the caller's static allowlist.
    pub async fn get_content_security_policies_page(
        &self,
        user_id: UserId,
        params: &ListParams,
        sort_col: &str,
    ) -> anyhow::Result<(Vec<ContentSecurityPolicy>, i64)> {
        let list = list_sql(
            "user_data_web_security_csp",
            "id, name, directives, created_at, updated_at",
            "name",
            &CSP_TAG_JUNCTION,
            sort_col,
            params.order,
        );
        let rows: Vec<RawContentSecurityPolicy> = sqlx::query_as(&list)
            .bind(*user_id)
            .bind(params.query.as_deref())
            .bind(params.tags.as_slice())
            .bind(params.global_tags.as_slice())
            .bind(params.limit)
            .bind(params.offset)
            .fetch_all(self.pool)
            .await?;

        let count = count_sql("user_data_web_security_csp", "name", &CSP_TAG_JUNCTION);
        let total: i64 = sqlx::query_scalar(&count)
            .bind(*user_id)
            .bind(params.query.as_deref())
            .bind(params.tags.as_slice())
            .bind(params.global_tags.as_slice())
            .fetch_one(self.pool)
            .await?;

        let mut policies = vec![];
        for raw_policy in rows {
            policies.push(ContentSecurityPolicy::try_from(raw_policy)?);
        }
        Ok((policies, total))
    }

    /// Fetches tags for a batch of content security policies.
    pub async fn get_csp_tags(
        &self,
        entity_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, Vec<EntityTag>>> {
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = query_as!(
            RawEntityTag,
            r#"
SELECT jt.csp_id AS entity_id, t.id, t.name, t.color
FROM user_data_web_security_csp_tags jt
JOIN user_tags t ON jt.tag_id = t.id
WHERE jt.csp_id = ANY($1)
ORDER BY t.name ASC
            "#,
            entity_ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(group_entity_tags(rows))
    }

    async fn set_csp_tags<'a>(
        executor: impl Acquire<'a, Database = Postgres>,
        entity_id: Uuid,
        tag_ids: &[Uuid],
    ) -> anyhow::Result<Vec<EntityTag>> {
        let mut conn = executor.acquire().await?;
        query!(
            "DELETE FROM user_data_web_security_csp_tags WHERE csp_id = $1",
            entity_id
        )
        .execute(&mut *conn)
        .await?;

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        Ok(query_as!(
            EntityTag,
            r#"
WITH inserted AS (
    INSERT INTO user_data_web_security_csp_tags (csp_id, tag_id)
    SELECT $1, unnest($2::uuid[])
    RETURNING csp_id, tag_id
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
    /// Returns a database extension for the web security utility-related operations.
    pub fn web_security(&self) -> WebSecurityDatabaseExt<'_> {
        WebSecurityDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error as SecutilsError,
        tests::{mock_user, mock_user_with_id},
        users::EntityTag,
        utils::web_security::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective,
            ContentSecurityPolicyTrustedTypesDirectiveValue,
        },
    };
    use actix_web::ResponseError;
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    fn get_mock_directives() -> anyhow::Result<Vec<ContentSecurityPolicyDirective>> {
        Ok(vec![
            ContentSecurityPolicyDirective::UpgradeInsecureRequests,
            ContentSecurityPolicyDirective::DefaultSrc(
                ["'self'".to_string(), "https://secutils.dev".to_string()]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::TrustedTypes(
                [ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates]
                    .into_iter()
                    .collect(),
            ),
        ])
    }

    #[sqlx::test]
    async fn can_add_and_retrieve_content_security_policies(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut content_security_policies = vec![
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for content_security_policy in content_security_policies.iter() {
            db.web_security()
                .insert_content_security_policy(user.id, content_security_policy)
                .await?;
        }

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, content_security_policies[0].id)
            .await?
            .unwrap();
        assert_eq!(content_security_policy, content_security_policies.remove(0));

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, content_security_policies[0].id)
            .await?
            .unwrap();
        assert_eq!(content_security_policy, content_security_policies.remove(0));

        assert!(
            db.web_security()
                .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000003"))
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_content_security_policies_on_insert(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let content_security_policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "csp-name".to_string(),
            directives: get_mock_directives()?,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };

        db.web_security()
            .insert_content_security_policy(user.id, &content_security_policy)
            .await?;

        let insert_error = db
            .web_security()
            .insert_content_security_policy(user.id, &content_security_policy)
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(insert_error.status_code(), 409);
        assert_debug_snapshot!(
            insert_error.root_cause.to_string(),
            @r###""Content security policy ('csp-name') already exists.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_content_security_policy_content(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        db.web_security()
            .insert_content_security_policy(
                user.id,
                &ContentSecurityPolicy {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "csp-name".to_string(),
                    directives: get_mock_directives()?,
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await?;

        db.web_security()
            .update_content_security_policy(
                user.id,
                &ContentSecurityPolicy {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "csp-name-new".to_string(),
                    directives: vec![ContentSecurityPolicyDirective::ReportTo([
                        "https://secutils.dev".to_string(),
                    ])],
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(956720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720820)?,
                },
                None,
            )
            .await?;

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            content_security_policy,
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name-new".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ReportTo([
                    "https://secutils.dev".to_string(),
                ])],
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720820)?,
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_content_security_policies_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let content_security_policy_a = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "csp-name-a".to_string(),
            directives: get_mock_directives()?,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.web_security()
            .insert_content_security_policy(user.id, &content_security_policy_a)
            .await?;

        let content_security_policy_b = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "csp-name-b".to_string(),
            directives: get_mock_directives()?,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.web_security()
            .insert_content_security_policy(user.id, &content_security_policy_b)
            .await?;

        let update_error = db
            .web_security()
            .update_content_security_policy(
                user.id,
                &ContentSecurityPolicy {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "csp-name-a".to_string(),
                    directives: get_mock_directives()?,
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
                None,
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(update_error.status_code(), 409);
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""Content security policy ('csp-name-a') already exists.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_non_existent_content_security_policies_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let update_error = db
            .web_security()
            .update_content_security_policy(
                user.id,
                &ContentSecurityPolicy {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "csp-name-a".to_string(),
                    directives: get_mock_directives()?,
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
                None,
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(update_error.status_code(), 400);
        assert_debug_snapshot!(
            update_error,
            @r###""A content security policy ('csp-name-a') doesn't exist.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_content_security_policies(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut content_security_policies = vec![
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for content_security_policy in content_security_policies.iter() {
            db.web_security()
                .insert_content_security_policy(user.id, content_security_policy)
                .await?;
        }

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(content_security_policy, content_security_policies.remove(0));

        let content_security_policy_2 = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(
            content_security_policy_2,
            content_security_policies[0].clone()
        );

        db.web_security()
            .remove_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(content_security_policy.is_none());

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(content_security_policy, content_security_policies.remove(0));

        db.web_security()
            .remove_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(content_security_policy.is_none());

        let content_security_policy = db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(content_security_policy.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_all_content_security_policies(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let content_security_policies = vec![
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for content_security_policy in content_security_policies.iter() {
            db.web_security()
                .insert_content_security_policy(user.id, content_security_policy)
                .await?;
        }

        assert_eq!(
            db.web_security()
                .get_content_security_policies(user.id)
                .await?,
            content_security_policies
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_content_security_policies_empty(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let policies = db
            .web_security()
            .bulk_get_content_security_policies(user.id, &[])
            .await?;
        assert!(policies.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_content_security_policies_returns_matching(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let policies = [
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000003"),
                name: "csp-name-3".to_string(),
                directives: get_mock_directives()?,
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946920800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946920810)?,
            },
        ];

        for policy in policies.iter() {
            db.web_security()
                .insert_content_security_policy(user.id, policy)
                .await?;
        }

        let result = db
            .web_security()
            .bulk_get_content_security_policies(
                user.id,
                &[
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    uuid!("00000000-0000-0000-0000-000000000003"),
                ],
            )
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], policies[0]);
        assert_eq!(result[1], policies[2]);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_content_security_policies_ignores_non_existent(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "csp-name".to_string(),
            directives: get_mock_directives()?,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.web_security()
            .insert_content_security_policy(user.id, &policy)
            .await?;

        let result = db
            .web_security()
            .bulk_get_content_security_policies(
                user.id,
                &[
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    uuid!("00000000-0000-0000-0000-000000000099"),
                ],
            )
            .await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], policy);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_content_security_policies_isolated_per_user(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "csp-name".to_string(),
            directives: get_mock_directives()?,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.web_security()
            .insert_content_security_policy(user_a.id, &policy)
            .await?;

        let result = db
            .web_security()
            .bulk_get_content_security_policies(
                user_b.id,
                &[uuid!("00000000-0000-0000-0000-000000000001")],
            )
            .await?;
        assert!(result.is_empty());

        Ok(())
    }

    // ── CSP tag tests ───────────────────────────────────────────────────

    fn mock_csp(
        id: uuid::Uuid,
        name: &str,
        tags: Vec<EntityTag>,
    ) -> anyhow::Result<ContentSecurityPolicy> {
        Ok(ContentSecurityPolicy {
            id,
            name: name.to_string(),
            directives: get_mock_directives()?,
            tags,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        })
    }

    #[sqlx::test]
    async fn can_set_and_get_csp_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag_a.id), EntityTag::from(tag_b.id)],
        )?;

        let web_security = db.web_security();
        let tags = web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags,
            vec![
                EntityTag {
                    id: tag_a.id,
                    name: "alpha".to_string(),
                    color: "primary".to_string()
                },
                EntityTag {
                    id: tag_b.id,
                    name: "beta".to_string(),
                    color: "danger".to_string()
                },
            ]
        );

        let tags_map = web_security.get_csp_tags(&[policy.id]).await?;
        assert_eq!(tags_map[&policy.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn update_csp_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag_a.id), EntityTag::from(tag_b.id)],
        )?;

        let web_security = db.web_security();
        web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        let tags = web_security
            .update_content_security_policy(user.id, &policy, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let tags_map = web_security.get_csp_tags(&[policy.id]).await?;
        let tag_names: Vec<&str> = tags_map[&policy.id]
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(tag_names, vec!["beta", "gamma"]);

        Ok(())
    }

    #[sqlx::test]
    async fn update_csp_clears_all_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag.id)],
        )?;

        let web_security = db.web_security();
        web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        let tags = web_security
            .update_content_security_policy(user.id, &policy, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = web_security.get_csp_tags(&[policy.id]).await?;
        assert!(!tags_map.contains_key(&policy.id) || tags_map[&policy.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_with_nonexistent_tag_ids_fails(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(uuid!(
                "00000000-0000-0000-0000-000000000099"
            ))],
        )?;

        let web_security = db.web_security();
        let result = web_security
            .insert_content_security_policy(user.id, &policy)
            .await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_returns_tags_ordered_by_name(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_z = db.insert_user_tag(user.id, "zebra", "primary").await?;
        let tag_a = db.insert_user_tag(user.id, "alpha", "danger").await?;
        let tag_m = db.insert_user_tag(user.id, "middle", "success").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![
                EntityTag::from(tag_z.id),
                EntityTag::from(tag_a.id),
                EntityTag::from(tag_m.id),
            ],
        )?;

        let web_security = db.web_security();
        let tags = web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_with_tags_is_atomic(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag_a.id), EntityTag::from(tag_b.id)],
        )?;

        let web_security = db.web_security();
        let tags = web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[1].name, "beta");

        let fetched = web_security
            .get_content_security_policy(user.id, policy.id)
            .await?;
        assert!(fetched.is_some());

        let tags_map = web_security.get_csp_tags(&[policy.id]).await?;
        assert_eq!(tags_map[&policy.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_with_tags_empty_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![],
        )?;

        let web_security = db.web_security();
        let tags = web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        assert!(tags.is_empty());

        let fetched = web_security
            .get_content_security_policy(user.id, policy.id)
            .await?;
        assert!(fetched.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn update_csp_with_tags_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag_a.id), EntityTag::from(tag_b.id)],
        )?;

        let web_security = db.web_security();
        web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        let updated = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "updated-name",
            vec![],
        )?;
        let tags = web_security
            .update_content_security_policy(user.id, &updated, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let fetched = web_security
            .get_content_security_policy(user.id, policy.id)
            .await?
            .unwrap();
        assert_eq!(fetched.name, "updated-name");

        Ok(())
    }

    #[sqlx::test]
    async fn update_csp_with_tags_clears_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag.id)],
        )?;

        let web_security = db.web_security();
        web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        let tags = web_security
            .update_content_security_policy(user.id, &policy, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = web_security.get_csp_tags(&[policy.id]).await?;
        assert!(!tags_map.contains_key(&policy.id) || tags_map[&policy.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_with_tags_rolls_back_on_invalid_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(uuid!(
                "00000000-0000-0000-0000-000000000099"
            ))],
        )?;

        let web_security = db.web_security();
        let result = web_security
            .insert_content_security_policy(user.id, &policy)
            .await;
        assert!(result.is_err());

        let fetched = web_security
            .get_content_security_policy(user.id, policy.id)
            .await?;
        assert!(fetched.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_with_tags_handles_duplicate_tag_ids(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag.id), EntityTag::from(tag.id)],
        )?;

        let web_security = db.web_security();
        let result = web_security
            .insert_content_security_policy(user.id, &policy)
            .await;

        match result {
            Ok(tags) => {
                assert_eq!(tags.len(), 1);
                assert_eq!(tags[0].name, "alpha");
            }
            Err(_) => {
                let fetched = web_security
                    .get_content_security_policy(user.id, policy.id)
                    .await?;
                assert!(fetched.is_none());
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn update_csp_with_tags_rolls_back_on_invalid_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let policy = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-name",
            vec![EntityTag::from(tag.id)],
        )?;

        let web_security = db.web_security();
        web_security
            .insert_content_security_policy(user.id, &policy)
            .await?;

        let updated = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "updated-name",
            vec![],
        )?;
        let result = web_security
            .update_content_security_policy(
                user.id,
                &updated,
                Some(vec![uuid!("00000000-0000-0000-0000-000000000099")]),
            )
            .await;
        assert!(result.is_err());

        let fetched = web_security
            .get_content_security_policy(user.id, policy.id)
            .await?
            .unwrap();
        assert_eq!(fetched.name, "csp-name");

        let tags_map = web_security.get_csp_tags(&[policy.id]).await?;
        assert_eq!(tags_map[&policy.id].len(), 1);
        assert_eq!(tags_map[&policy.id][0].name, "alpha");

        Ok(())
    }

    #[sqlx::test]
    async fn insert_csp_with_tags_isolated_between_users(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let tag_a = db.insert_user_tag(user_a.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user_b.id, "beta", "danger").await?;

        let policy_a = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "csp-a",
            vec![EntityTag::from(tag_a.id)],
        )?;
        let policy_b = mock_csp(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "csp-b",
            vec![EntityTag::from(tag_b.id)],
        )?;

        let web_security = db.web_security();
        let tags_a = web_security
            .insert_content_security_policy(user_a.id, &policy_a)
            .await?;
        let tags_b = web_security
            .insert_content_security_policy(user_b.id, &policy_b)
            .await?;

        assert_eq!(tags_a.len(), 1);
        assert_eq!(tags_a[0].name, "alpha");
        assert_eq!(tags_b.len(), 1);
        assert_eq!(tags_b[0].name, "beta");

        let map_a = web_security.get_csp_tags(&[policy_a.id]).await?;
        let map_b = web_security.get_csp_tags(&[policy_b.id]).await?;
        assert_eq!(map_a[&policy_a.id].len(), 1);
        assert_eq!(map_b[&policy_b.id].len(), 1);
        assert_eq!(map_a[&policy_a.id][0].name, "alpha");
        assert_eq!(map_b[&policy_b.id][0].name, "beta");

        Ok(())
    }
}
