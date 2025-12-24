mod raw_user;
mod raw_user_data;
mod raw_user_share;

use self::{raw_user::RawUser, raw_user_data::RawUserData, raw_user_share::RawUserShare};
use crate::{
    database::Database,
    users::{SharedResource, User, UserData, UserDataKey, UserId, UserShare, UserShareId},
};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, query_scalar};

/// Extends primary database with the user management-related methods.
impl Database {
    /// Retrieves user from the `Users` table using user ID.
    pub async fn get_user(&self, id: UserId) -> anyhow::Result<Option<User>> {
        query_as!(
            RawUser,
            r#"
SELECT id, email, handle, created_at, s.tier as subscription_tier,
       s.started_at as subscription_started_at, s.ends_at as subscription_ends_at,
       s.trial_started_at as subscription_trial_started_at,
       s.trial_ends_at as subscription_trial_ends_at
FROM users as u
INNER JOIN user_subscriptions as s
ON s.user_id = u.id
WHERE u.id = $1
                "#,
            *id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(User::try_from)
        .transpose()
    }

    /// Retrieves user from the `Users` table using user email.
    pub async fn get_user_by_email<T: AsRef<str>>(&self, email: T) -> anyhow::Result<Option<User>> {
        let email = email.as_ref();
        query_as!(
            RawUser,
            r#"
SELECT id, email, handle, created_at, s.tier as subscription_tier,
       s.started_at as subscription_started_at, s.ends_at as subscription_ends_at,
       s.trial_started_at as subscription_trial_started_at,
       s.trial_ends_at as subscription_trial_ends_at
FROM users as u
INNER JOIN user_subscriptions as s
ON s.user_id = u.id
WHERE u.email = $1
                "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .map(User::try_from)
        .transpose()
    }

    /// Retrieves user from the `Users` index using user handle.
    pub async fn get_user_by_handle<T: AsRef<str>>(
        &self,
        handle: T,
    ) -> anyhow::Result<Option<User>> {
        let handle = handle.as_ref();
        let mut raw_users = query_as!(
            RawUser,
            r#"
SELECT id, email, handle, created_at, s.tier as subscription_tier,
       s.started_at as subscription_started_at, s.ends_at as subscription_ends_at,
       s.trial_started_at as subscription_trial_started_at,
       s.trial_ends_at as subscription_trial_ends_at
FROM users as u
INNER JOIN user_subscriptions as s
ON s.user_id = u.id
WHERE u.handle = $1
             "#,
            handle
        )
        .fetch_all(&self.pool)
        .await?;

        if raw_users.is_empty() {
            return Ok(None);
        }

        if raw_users.len() > 1 {
            bail!(
                "Founds {} users for the same handle {}.",
                raw_users.len(),
                handle
            );
        }

        raw_users.pop().map(User::try_from).transpose()
    }

    /// Inserts user to the `Users` tables, fails if user already exists.
    pub async fn insert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<()> {
        let raw_user = RawUser::from(user.as_ref());

        let tx = self.pool.begin().await?;

        // Insert user.
        query!(
            r#"
INSERT INTO users (id, email, handle, created_at)
VALUES ( $1, $2, $3, $4 )
        "#,
            raw_user.id,
            &raw_user.email,
            &raw_user.handle,
            raw_user.created_at
        )
        .execute(&self.pool)
        .await?;

        // Insert user subscription.
        query!(
            r#"
INSERT INTO user_subscriptions (user_id, tier, started_at, ends_at, trial_started_at, trial_ends_at)
VALUES ( $1, $2, $3, $4, $5, $6 )
        "#,
            raw_user.id,
            raw_user.subscription_tier,
            raw_user.subscription_started_at,
            raw_user.subscription_ends_at,
            raw_user.subscription_trial_started_at,
            raw_user.subscription_trial_ends_at
        )
        .execute(&self.pool)
        .await?;

        Ok(tx.commit().await?)
    }

    /// Inserts or updates user in the `Users` table.
    pub async fn upsert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<()> {
        let raw_user = RawUser::from(user.as_ref());

        let tx = self.pool.begin().await?;

        // Update user
        query!(r#"
INSERT INTO users (id, email, handle, created_at)
VALUES ( $1, $2, $3, $4 )
ON CONFLICT(id) DO UPDATE SET email=excluded.email, handle=excluded.handle, created_at=excluded.created_at
        "#,
            raw_user.id,
            &raw_user.email,
            &raw_user.handle,
            raw_user.created_at
        )
            .execute(&self.pool)
            .await?;

        // Update user subscription.
        query!(
            r#"
INSERT INTO user_subscriptions (user_id, tier, started_at, ends_at, trial_started_at, trial_ends_at)
VALUES ( $1, $2, $3, $4, $5, $6 )
ON CONFLICT(user_id) DO UPDATE SET tier=excluded.tier, started_at=excluded.started_at, ends_at=excluded.ends_at, trial_started_at=excluded.trial_started_at, trial_ends_at=excluded.trial_ends_at
        "#,
            raw_user.id,
            raw_user.subscription_tier,
            raw_user.subscription_started_at,
            raw_user.subscription_ends_at,
            raw_user.subscription_trial_started_at,
            raw_user.subscription_trial_ends_at
        )
            .execute(&self.pool)
            .await?;

        Ok(tx.commit().await?)
    }

    /// Removes user with the specified email from the `Users` table.
    pub async fn remove_user_by_email<T: AsRef<str>>(
        &self,
        email: T,
    ) -> anyhow::Result<Option<UserId>> {
        let email = email.as_ref();
        Ok(query_scalar!(
            r#"
DELETE FROM users
WHERE email = $1
RETURNING id
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserId::from))
    }

    /// Retrieves user data from the `UserData` table using user id and data key.
    pub async fn get_user_data<R: for<'de> Deserialize<'de>>(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<Option<UserData<R>>> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        query_as!(
            RawUserData,
            r#"
SELECT user_id, key, value, timestamp
FROM user_data
WHERE user_id = $1 AND namespace = $2 AND key = $3
                "#,
            *user_id,
            namespace,
            key
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserData::try_from)
        .transpose()
    }

    /// Sets user data in the `UserData` table using user id and data key.
    pub async fn upsert_user_data<R: Serialize>(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<R>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        let raw_user_data = RawUserData::try_from(&user_data)?;
        query!(
            r#"
INSERT INTO user_data (user_id, namespace, key, value, timestamp)
VALUES ( $1, $2, $3, $4, $5 )
ON CONFLICT(user_id, namespace, key) DO UPDATE SET value=excluded.value, timestamp=excluded.timestamp
        "#,
            raw_user_data.user_id,
            namespace,
            key,
            raw_user_data.value,
            raw_user_data.timestamp
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Deletes user data from the `UserData` table using user id and data key.
    pub async fn remove_user_data(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        query!(
            r#"
DELETE FROM user_data
WHERE user_id = $1 AND namespace = $2 AND key = $3
            "#,
            *user_id,
            namespace,
            key
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves user share from `user_shares` table using user share ID.
    pub async fn get_user_share(&self, id: UserShareId) -> anyhow::Result<Option<UserShare>> {
        query_as!(
            RawUserShare,
            r#"
SELECT id, user_id, resource, created_at
FROM user_shares
WHERE id = $1
                "#,
            *id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserShare::try_from)
        .transpose()
    }

    /// Retrieves user share from `user_shares` table using user ID and resource.
    pub async fn get_user_share_by_resource(
        &self,
        user_id: UserId,
        resource: &SharedResource,
    ) -> anyhow::Result<Option<UserShare>> {
        let resource = postcard::to_stdvec(resource)?;
        query_as!(
            RawUserShare,
            r#"
SELECT id, user_id, resource, created_at
FROM user_shares
WHERE user_id = $1 AND resource = $2
                "#,
            *user_id,
            resource
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserShare::try_from)
        .transpose()
    }

    /// Inserts user share to the `user_shares` table.
    pub async fn insert_user_share(&self, user_share: &UserShare) -> anyhow::Result<()> {
        let raw_user_share = RawUserShare::try_from(user_share)?;

        query!(
            r#"
INSERT INTO user_shares (id, user_id, resource, created_at)
VALUES ($1, $2, $3, $4)
        "#,
            raw_user_share.id,
            raw_user_share.user_id,
            raw_user_share.resource,
            raw_user_share.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Removes user share from the `user_shares` table using user share ID and returns removed
    /// user share object if it was found.
    pub async fn remove_user_share(&self, id: UserShareId) -> anyhow::Result<Option<UserShare>> {
        query_as!(
            RawUserShare,
            r#"
DELETE FROM user_shares
WHERE id = $1
RETURNING id as "id!", user_id as "user_id!", resource as "resource!", created_at as "created_at!"
            "#,
            *id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserShare::try_from)
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        tests::{MockUserBuilder, mock_user_with_id, to_database_error},
        users::{
            SharedResource, SubscriptionTier, UserData, UserDataNamespace, UserId, UserShare,
            UserShareId, UserSubscription,
        },
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_add_and_retrieve_users(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        assert!(db.get_user_by_email("some-id").await?.is_none());

        let users = vec![
            MockUserBuilder::new(
                uuid!("00000000-0000-0000-0000-000000000001").into(),
                "dev@secutils.dev",
                "devhandle",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_is_activated()
            .build(),
            MockUserBuilder::new(
                uuid!("00000000-0000-0000-0000-000000000002").into(),
                "prod@secutils.dev",
                "prod-handle",
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_subscription(UserSubscription {
                tier: SubscriptionTier::Standard,
                started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            })
            .build(),
            MockUserBuilder::new(
                uuid!("00000000-0000-0000-0000-000000000003").into(),
                "user@secutils.dev",
                "handle",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_subscription(UserSubscription {
                tier: SubscriptionTier::Professional,
                started_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                ends_at: Some(OffsetDateTime::from_unix_timestamp(946720801)?),
                trial_started_at: Some(OffsetDateTime::from_unix_timestamp(946720802)?),
                trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720803)?),
            })
            .build(),
        ];
        for user in users {
            db.upsert_user(&user).await?;
        }

        let user_by_id = db
            .get_user(uuid!("00000000-0000-0000-0000-000000000001").into())
            .await?
            .unwrap();
        let user_by_email = db.get_user_by_email("dev@secutils.dev").await?.unwrap();
        assert_eq!(user_by_id.id, user_by_email.id);
        assert_debug_snapshot!(user_by_email, @r###"
        User {
            id: UserId(
                00000000-0000-0000-0000-000000000001,
            ),
            email: "dev@secutils.dev",
            handle: "devhandle",
            created_at: 2000-01-01 10:00:00.0 +00:00:00,
            is_activated: false,
            is_operator: false,
            subscription: UserSubscription {
                tier: Ultimate,
                started_at: 2000-01-01 10:00:01.0 +00:00:00,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            },
        }
        "###);

        let user_by_id = db
            .get_user(uuid!("00000000-0000-0000-0000-000000000002").into())
            .await?
            .unwrap();
        let user_by_email = db.get_user_by_email("prod@secutils.dev").await?.unwrap();
        assert_eq!(user_by_id.id, user_by_email.id);
        assert_debug_snapshot!(user_by_email, @r###"
        User {
            id: UserId(
                00000000-0000-0000-0000-000000000002,
            ),
            email: "prod@secutils.dev",
            handle: "prod-handle",
            created_at: 2010-01-01 10:00:00.0 +00:00:00,
            is_activated: false,
            is_operator: false,
            subscription: UserSubscription {
                tier: Standard,
                started_at: 2010-01-01 10:00:00.0 +00:00:00,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            },
        }
        "###);

        let user_by_id = db
            .get_user(uuid!("00000000-0000-0000-0000-000000000003").into())
            .await?
            .unwrap();
        let user_by_email = db.get_user_by_email("user@secutils.dev").await?.unwrap();
        assert_eq!(user_by_id.id, user_by_email.id);
        assert_debug_snapshot!(user_by_email, @r###"
        User {
            id: UserId(
                00000000-0000-0000-0000-000000000003,
            ),
            email: "user@secutils.dev",
            handle: "handle",
            created_at: 2000-01-01 10:00:00.0 +00:00:00,
            is_activated: false,
            is_operator: false,
            subscription: UserSubscription {
                tier: Professional,
                started_at: 2000-01-01 10:00:00.0 +00:00:00,
                ends_at: Some(
                    2000-01-01 10:00:01.0 +00:00:00,
                ),
                trial_started_at: Some(
                    2000-01-01 10:00:02.0 +00:00:00,
                ),
                trial_ends_at: Some(
                    2000-01-01 10:00:03.0 +00:00:00,
                ),
            },
        }
        "###);

        assert!(
            db.get_user_by_email("unknown@secutils.dev")
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn ignores_email_case(pool: PgPool) -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "DeV@secutils.dev",
            "DeVhandle",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Professional,
            started_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        })
        .set_is_activated()
        .build();
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?,  @r###"
        Some(
            User {
                id: UserId(
                    00000000-0000-0000-0000-000000000001,
                ),
                email: "DeV@secutils.dev",
                handle: "DeVhandle",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
                is_activated: false,
                is_operator: false,
                subscription: UserSubscription {
                    tier: Professional,
                    started_at: 2000-01-01 10:00:00.0 +00:00:00,
                    ends_at: None,
                    trial_started_at: None,
                    trial_ends_at: None,
                },
            },
        )
        "###);
        assert_eq!(
            db.get_user_by_email("DEV@secutils.dev").await?.unwrap().id,
            user.id
        );
        assert_eq!(
            db.get_user_by_email("DeV@secutils.dev").await?.unwrap().id,
            user.id
        );

        Ok(())
    }

    #[sqlx::test]
    async fn ignores_handle_case(pool: PgPool) -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "DeV@secutils.dev",
            "DeVhandle",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_is_activated()
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Professional,
            started_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        })
        .build();
        let db = Database::create(pool).await?;
        db.upsert_user(&user).await?;

        assert_debug_snapshot!(db.get_user_by_handle("devhandle").await?,  @r###"
        Some(
            User {
                id: UserId(
                    00000000-0000-0000-0000-000000000001,
                ),
                email: "DeV@secutils.dev",
                handle: "DeVhandle",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
                is_activated: false,
                is_operator: false,
                subscription: UserSubscription {
                    tier: Professional,
                    started_at: 2000-01-01 10:00:00.0 +00:00:00,
                    ends_at: None,
                    trial_started_at: None,
                    trial_ends_at: None,
                },
            },
        )
        "###);
        assert_eq!(
            db.get_user_by_handle("DEVhandle").await?.unwrap().id,
            user.id
        );
        assert_eq!(
            db.get_user_by_handle("DeVhandle").await?.unwrap().id,
            user.id
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_insert_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        let user = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "dev@secutils.dev",
            "devhandle",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_is_activated()
        .build();
        db.insert_user(&user).await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    00000000-0000-0000-0000-000000000001,
                ),
                email: "dev@secutils.dev",
                handle: "devhandle",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
                is_activated: false,
                is_operator: false,
                subscription: UserSubscription {
                    tier: Ultimate,
                    started_at: 2000-01-01 10:00:01.0 +00:00:00,
                    ends_at: None,
                    trial_started_at: None,
                    trial_ends_at: None,
                },
            },
        )
        "###);

        let conflict_error = to_database_error(
            db.insert_user(
                &MockUserBuilder::new(
                    uuid!("00000000-0000-0000-0000-000000000100").into(),
                    "DEV@secutils.dev",
                    "DEVhandle",
                    // January 1, 2000 11:00:00
                    OffsetDateTime::from_unix_timestamp(1262340000)?,
                )
                .build(),
            )
            .await
            .unwrap_err(),
        )?;
        assert_debug_snapshot!(conflict_error.message(), @r###""duplicate key value violates unique constraint \"users_email_key\"""###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            user.id
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        db.upsert_user(
            &MockUserBuilder::new(
                uuid!("00000000-0000-0000-0000-000000000001").into(),
                "dev@secutils.dev",
                "devhandle",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_is_activated()
            .build(),
        )
        .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    00000000-0000-0000-0000-000000000001,
                ),
                email: "dev@secutils.dev",
                handle: "devhandle",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
                is_activated: false,
                is_operator: false,
                subscription: UserSubscription {
                    tier: Ultimate,
                    started_at: 2000-01-01 10:00:01.0 +00:00:00,
                    ends_at: None,
                    trial_started_at: None,
                    trial_ends_at: None,
                },
            },
        )
        "###);

        db.upsert_user(
            &MockUserBuilder::new(
                uuid!("00000000-0000-0000-0000-000000000001").into(),
                "DEV@secutils.dev",
                "DEVhandle",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_subscription(UserSubscription {
                tier: SubscriptionTier::Basic,
                started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            })
            .build(),
        )
        .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    00000000-0000-0000-0000-000000000001,
                ),
                email: "DEV@secutils.dev",
                handle: "DEVhandle",
                created_at: 2010-01-01 10:00:00.0 +00:00:00,
                is_activated: false,
                is_operator: false,
                subscription: UserSubscription {
                    tier: Basic,
                    started_at: 2010-01-01 10:00:00.0 +00:00:00,
                    ends_at: None,
                    trial_started_at: None,
                    trial_ends_at: None,
                },
            },
        )
        "###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            db.get_user_by_email("DEV@secutils.dev").await?.unwrap().id
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());

        let user_dev = MockUserBuilder::new(
            UserId::new(),
            "dev@secutils.dev",
            "devhandle",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_is_activated()
        .build();
        let user_prod = MockUserBuilder::new(
            UserId::new(),
            "prod@secutils.dev",
            "prod-handle",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        db.upsert_user(&user_dev).await?;
        db.upsert_user(&user_prod).await?;

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            user_dev.id
        );
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            user_prod.id
        );

        assert_eq!(
            db.remove_user_by_email("dev@secutils.dev").await?.unwrap(),
            user_dev.id
        );
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.remove_user_by_email("dev@secutils.dev").await?.is_none());
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            user_prod.id
        );

        assert_eq!(
            db.remove_user_by_email("prod@secutils.dev").await?.unwrap(),
            user_prod.id
        );
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());
        assert!(
            db.remove_user_by_email("prod@secutils.dev")
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_manipulate_user_data(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "dev@secutils.dev",
            "devhandle",
            OffsetDateTime::now_utc(),
        )
        .set_is_activated()
        .build();

        // No user and no data yet.
        assert_eq!(
            db.get_user_data::<String>(user.id, UserDataNamespace::UserSettings)
                .await?,
            None
        );

        db.upsert_user(&user).await?;

        // Nodata yet.
        assert_eq!(
            db.get_user_data::<String>(user.id, UserDataNamespace::UserSettings)
                .await?,
            None
        );

        // Insert data.
        db.upsert_user_data(
            UserDataNamespace::UserSettings,
            UserData::new(
                user.id,
                "data",
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, UserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                user.id,
                "data".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Update data.
        db.upsert_user_data(
            UserDataNamespace::UserSettings,
            UserData::new(
                user.id,
                "data-new",
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, UserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                user.id,
                "data-new".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Remove data.
        db.remove_user_data(user.id, UserDataNamespace::UserSettings)
            .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, UserDataNamespace::UserSettings)
                .await?,
            None
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_add_and_retrieve_user_shares(pool: PgPool) -> anyhow::Result<()> {
        let user_shares = vec![
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000002")),
                user_id: uuid!("00000000-0000-0000-0000-000000000002").into(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000002"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let db = Database::create(pool).await?;
        db.insert_user(mock_user_with_id(uuid!(
            "00000000-0000-0000-0000-000000000001"
        ))?)
        .await?;
        db.insert_user(mock_user_with_id(uuid!(
            "00000000-0000-0000-0000-000000000002"
        ))?)
        .await?;

        for user_share in user_shares.iter() {
            assert!(db.get_user_share(user_share.id).await?.is_none());
        }

        // 1. Insert new user shares.
        for user_share in user_shares.iter() {
            db.insert_user_share(user_share).await?;
        }

        // 2. Make sure they were inserted correctly.
        for user_share in user_shares {
            assert_eq!(
                db.get_user_share(user_share.id).await?,
                Some(user_share.clone())
            );
        }

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_user_shares_by_resource(pool: PgPool) -> anyhow::Result<()> {
        let user_shares = [
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000002")),
                user_id: uuid!("00000000-0000-0000-0000-000000000002").into(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000002"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let db = Database::create(pool).await?;
        db.insert_user(mock_user_with_id(uuid!(
            "00000000-0000-0000-0000-000000000001"
        ))?)
        .await?;
        db.insert_user(mock_user_with_id(uuid!(
            "00000000-0000-0000-0000-000000000002"
        ))?)
        .await?;

        // 1. Insert new user shares.
        for user_share in user_shares.iter() {
            db.insert_user_share(user_share).await?;
        }

        assert_eq!(
            db.get_user_share_by_resource(user_shares[0].user_id, &user_shares[0].resource)
                .await?,
            Some(user_shares[0].clone())
        );
        assert_eq!(
            db.get_user_share_by_resource(user_shares[1].user_id, &user_shares[1].resource)
                .await?,
            Some(user_shares[1].clone())
        );

        assert!(
            db.get_user_share_by_resource(
                uuid!("00000000-0000-0000-0000-000000000003").into(),
                &user_shares[0].resource
            )
            .await?
            .is_none()
        );
        assert!(
            db.get_user_share_by_resource(
                user_shares[0].user_id,
                &SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000003"
                ))
            )
            .await?
            .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_user_shares(pool: PgPool) -> anyhow::Result<()> {
        let user_shares = vec![
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000002")),
                user_id: uuid!("00000000-0000-0000-0000-000000000002").into(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000002"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let db = Database::create(pool).await?;
        db.insert_user(mock_user_with_id(uuid!(
            "00000000-0000-0000-0000-000000000001"
        ))?)
        .await?;
        db.insert_user(mock_user_with_id(uuid!(
            "00000000-0000-0000-0000-000000000002"
        ))?)
        .await?;

        for user_share in user_shares.iter() {
            assert!(db.get_user_share(user_share.id).await?.is_none());
        }

        // 1. Insert new user shares.
        for user_share in user_shares.iter() {
            db.insert_user_share(user_share).await?;
        }

        // 2. Make sure they were inserted correctly.
        for user_share in user_shares.iter() {
            assert_eq!(
                db.get_user_share(user_share.id).await?,
                Some(user_share.clone())
            );
        }

        // 3. Remove the first user share.
        assert_eq!(
            db.remove_user_share(user_shares[0].id).await?,
            Some(user_shares[0].clone())
        );
        assert!(db.get_user_share(user_shares[0].id).await?.is_none());
        assert_eq!(
            db.get_user_share(user_shares[1].id).await?,
            Some(user_shares[1].clone())
        );

        // 3. Remove the last user share.
        assert_eq!(
            db.remove_user_share(user_shares[1].id).await?,
            Some(user_shares[1].clone())
        );
        for user_share in user_shares {
            assert!(db.get_user_share(user_share.id).await?.is_none());
        }

        Ok(())
    }
}
