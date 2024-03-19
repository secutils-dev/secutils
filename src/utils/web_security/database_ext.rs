mod raw_content_security_policy;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::UserId,
    utils::web_security::{
        database_ext::raw_content_security_policy::RawContentSecurityPolicy, ContentSecurityPolicy,
    },
};
use anyhow::{anyhow, bail};
use sqlx::{error::ErrorKind as SqlxErrorKind, query, query_as, Pool, Sqlite};
use uuid::Uuid;

/// A database extension for the web security utility-related operations.
pub struct WebSecurityDatabaseExt<'pool> {
    pool: &'pool Pool<Sqlite>,
}

impl<'pool> WebSecurityDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Retrieves content security policy for the specified user with the specified ID.
    pub async fn get_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<ContentSecurityPolicy>> {
        let id: &[u8] = id.as_ref();
        query_as!(
            RawContentSecurityPolicy,
            r#"
SELECT id, name, directives, created_at
FROM user_data_web_security_csp
WHERE user_id = ?1 AND id = ?2
                "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(ContentSecurityPolicy::try_from)
        .transpose()
    }

    /// Inserts content security policy.
    pub async fn insert_content_security_policy(
        &self,
        user_id: UserId,
        policy: &ContentSecurityPolicy,
    ) -> anyhow::Result<()> {
        let raw_policy = RawContentSecurityPolicy::try_from(policy)?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_security_csp (user_id, id, name, directives, created_at)
    VALUES ( ?1, ?2, ?3, ?4, ?5 )
            "#,
            *user_id,
            raw_policy.id,
            raw_policy.name,
            raw_policy.directives,
            raw_policy.created_at
        )
        .execute(self.pool)
        .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Content security policy ('{}') already exists.",
                    policy.name
                )))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create content security policy ('{}') due to unknown reason.",
                    policy.name
                )))
            });
        }

        Ok(())
    }

    /// Updates content security policy.
    pub async fn update_content_security_policy(
        &self,
        user_id: UserId,
        policy: &ContentSecurityPolicy,
    ) -> anyhow::Result<()> {
        let raw_policy = RawContentSecurityPolicy::try_from(policy)?;
        let result = query!(
            r#"
    UPDATE user_data_web_security_csp
    SET name = ?3, directives = ?4
    WHERE user_id = ?1 AND id = ?2
            "#,
            *user_id,
            raw_policy.id,
            raw_policy.name,
            raw_policy.directives
        )
        .execute(self.pool)
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
                    SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                        "Content security policy ('{}') already exists.",
                        policy.name
                    )))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update content security policy ('{}') due to unknown reason.",
                        policy.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes content security policy for the specified user with the specified ID.
    pub async fn remove_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<()> {
        let id: &[u8] = id.as_ref();
        query!(
            r#"
    DELETE FROM user_data_web_security_csp
    WHERE user_id = ?1 AND id = ?2
                    "#,
            *user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all content security policies for the specified user.
    pub async fn get_content_security_policies(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<ContentSecurityPolicy>> {
        let raw_policies = query_as!(
            RawContentSecurityPolicy,
            r#"
    SELECT id, name, directives, created_at
    FROM user_data_web_security_csp
    WHERE user_id = ?1
    ORDER BY created_at
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
}

impl Database {
    /// Returns a database extension for the web security utility-related operations.
    pub fn web_security(&self) -> WebSecurityDatabaseExt {
        WebSecurityDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        tests::{mock_db, mock_user},
        utils::web_security::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective,
            ContentSecurityPolicyTrustedTypesDirectiveValue,
        },
    };
    use actix_web::ResponseError;
    use insta::assert_debug_snapshot;
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

    #[tokio::test]
    async fn can_add_and_retrieve_content_security_policies() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut content_security_policies = vec![
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
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

        assert!(db
            .web_security()
            .get_content_security_policy(user.id, uuid!("00000000-0000-0000-0000-000000000003"))
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_duplicated_content_security_policies_on_insert() -> anyhow::Result<()>
    {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let content_security_policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "csp-name".to_string(),
            directives: get_mock_directives()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
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
        assert_eq!(insert_error.status_code(), 400);
        assert_debug_snapshot!(
            insert_error,
            @r###"
        Error {
            context: "Content security policy (\'csp-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_security_csp.name, user_data_web_security_csp.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_update_content_security_policy_content() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        db.web_security()
            .insert_content_security_policy(
                user.id,
                &ContentSecurityPolicy {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "csp-name".to_string(),
                    directives: get_mock_directives()?,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
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
                    created_at: OffsetDateTime::from_unix_timestamp(956720800)?,
                },
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
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_duplicated_content_security_policies_on_update() -> anyhow::Result<()>
    {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let content_security_policy_a = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "csp-name-a".to_string(),
            directives: get_mock_directives()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };
        db.web_security()
            .insert_content_security_policy(user.id, &content_security_policy_a)
            .await?;

        let content_security_policy_b = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "csp-name-b".to_string(),
            directives: get_mock_directives()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
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
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(update_error.status_code(), 400);
        assert_debug_snapshot!(
            update_error,
            @r###"
        Error {
            context: "Content security policy (\'csp-name-a\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_security_csp.name, user_data_web_security_csp.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_non_existent_content_security_policies_on_update(
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let update_error = db
            .web_security()
            .update_content_security_policy(
                user.id,
                &ContentSecurityPolicy {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "csp-name-a".to_string(),
                    directives: get_mock_directives()?,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
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

    #[tokio::test]
    async fn can_remove_content_security_policies() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut content_security_policies = vec![
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
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

    #[tokio::test]
    async fn can_retrieve_all_content_security_policies() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let content_security_policies = vec![
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: get_mock_directives()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "csp-name-2".to_string(),
                directives: get_mock_directives()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
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
}
