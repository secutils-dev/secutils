mod raw_user;
mod raw_user_data;
mod raw_user_share;
mod raw_user_to_upsert;

use self::{
    raw_user::RawUser, raw_user_data::RawUserData, raw_user_share::RawUserShare,
    raw_user_to_upsert::RawUserToUpsert,
};
use crate::{
    database::Database,
    users::{SharedResource, User, UserData, UserDataKey, UserId, UserShare, UserShareId},
};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, query_scalar};
use time::OffsetDateTime;

/// Extends primary database with the user management-related methods.
impl Database {
    /// Retrieves user from the `Users` table using user ID.
    pub async fn get_user(&self, id: UserId) -> anyhow::Result<Option<User>> {
        query_as!(
            RawUser,
            r#"
SELECT id, email, handle, credentials, created, activated, s.tier as subscription_tier,
       s.started_at as subscription_started_at, s.ends_at as subscription_ends_at,
       s.trial_started_at as subscription_trial_started_at, 
       s.trial_ends_at as subscription_trial_ends_at
FROM users as u
INNER JOIN user_subscriptions as s
ON s.user_id = u.id
WHERE u.id = ?1
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
SELECT id, email, handle, credentials, created, activated, s.tier as subscription_tier,
       s.started_at as subscription_started_at, s.ends_at as subscription_ends_at,
       s.trial_started_at as subscription_trial_started_at, 
       s.trial_ends_at as subscription_trial_ends_at
FROM users as u
INNER JOIN user_subscriptions as s
ON s.user_id = u.id
WHERE u.email = ?1
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
SELECT id, email, handle, credentials, created, activated, s.tier as subscription_tier,
       s.started_at as subscription_started_at, s.ends_at as subscription_ends_at,
       s.trial_started_at as subscription_trial_started_at, 
       s.trial_ends_at as subscription_trial_ends_at
FROM users as u
INNER JOIN user_subscriptions as s
ON s.user_id = u.id
WHERE u.handle = ?1
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
                raw_users.len().to_string(),
                handle
            );
        }

        raw_users.pop().map(User::try_from).transpose()
    }

    /// Inserts user to the `Users` tables, fails if user already exists.
    pub async fn insert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        let raw_user = RawUserToUpsert::try_from(user.as_ref())?;

        let tx = self.pool.begin().await?;

        // Insert user.
        let user_id: i64 = query_scalar!(
            r#"
INSERT INTO users (email, handle, credentials, created, activated)
VALUES ( ?1, ?2, ?3, ?4, ?5 )
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.credentials,
            raw_user.created,
            raw_user.activated
        )
        .fetch_one(&self.pool)
        .await?;

        // Insert user subscription.
        query!(
            r#"
INSERT INTO user_subscriptions (user_id, tier, started_at, ends_at, trial_started_at, trial_ends_at)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
        "#,
            user_id,
            raw_user.subscription_tier,
            raw_user.subscription_started_at,
            raw_user.subscription_ends_at,
            raw_user.subscription_trial_started_at,
            raw_user.subscription_trial_ends_at
        )
        .execute(&self.pool)
        .await?;

        tx.commit().await?;

        user_id.try_into()
    }

    /// Inserts or updates user in the `Users` table.
    pub async fn upsert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        let raw_user = RawUserToUpsert::try_from(user.as_ref())?;

        let tx = self.pool.begin().await?;

        // Update user
        let user_id: i64 = query_scalar!(r#"
INSERT INTO users (email, handle, credentials, created, activated)
VALUES ( ?1, ?2, ?3, ?4, ?5 )
ON CONFLICT(email) DO UPDATE SET handle=excluded.handle, credentials=excluded.credentials, created=excluded.created, activated=excluded.activated 
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.credentials,
            raw_user.created,
            raw_user.activated
        )
            .fetch_one(&self.pool)
            .await?;

        // Update user subscription.
        query!(
            r#"
INSERT INTO user_subscriptions (user_id, tier, started_at, ends_at, trial_started_at, trial_ends_at)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
ON CONFLICT(user_id) DO UPDATE SET tier=excluded.tier, started_at=excluded.started_at, ends_at=excluded.ends_at, trial_started_at=excluded.trial_started_at, trial_ends_at=excluded.trial_ends_at
        "#,
            user_id,
            raw_user.subscription_tier,
            raw_user.subscription_started_at,
            raw_user.subscription_ends_at,
            raw_user.subscription_trial_started_at,
            raw_user.subscription_trial_ends_at
        )
            .execute(&self.pool)
            .await?;

        tx.commit().await?;

        user_id.try_into()
    }

    /// Removes user with the specified email from the `Users` table.
    pub async fn remove_user_by_email<T: AsRef<str>>(
        &self,
        email: T,
    ) -> anyhow::Result<Option<UserId>> {
        let email = email.as_ref();
        query_scalar!(
            r#"
DELETE FROM users
WHERE email = ?1
RETURNING id as "id!"
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserId::try_from)
        .transpose()
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
WHERE user_id = ?1 AND namespace = ?2 AND key = ?3
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
VALUES ( ?1, ?2, ?3, ?4, ?5 )
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
WHERE user_id = ?1 AND namespace = ?2 AND key = ?3
            "#,
            *user_id,
            namespace,
            key
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user data from the `UserData` table with the specified data key if it's older than
    /// specified `since` timestamp.
    pub async fn cleanup_user_data(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
        since: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        let since = since.unix_timestamp();
        query!(
            r#"
DELETE FROM user_data
WHERE namespace = ?1 AND key = ?2 AND timestamp <= ?3
            "#,
            namespace,
            key,
            since
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
WHERE id = ?1
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
WHERE user_id = ?1 AND resource = ?2
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
VALUES (?1, ?2, ?3, ?4)
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
WHERE id = ?1
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
        security::StoredCredentials,
        tests::{mock_db, mock_user_with_id, MockUserBuilder},
        users::{
            InternalUserDataNamespace, PublicUserDataNamespace, SharedResource, SubscriptionTier,
            UserData, UserId, UserShare, UserShareId, UserSubscription,
        },
    };
    use insta::assert_debug_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[tokio::test]
    async fn can_add_and_retrieve_users() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_user_by_email("some-id").await?.is_none());

        let users = vec![
            MockUserBuilder::new(
                UserId::default(),
                "dev@secutils.dev",
                "dev-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_activated()
            .build(),
            MockUserBuilder::new(
                UserId::default(),
                "prod@secutils.dev",
                "prod-handle",
                StoredCredentials {
                    password_hash: Some("hash_prod".to_string()),
                    ..Default::default()
                },
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
                UserId::default(),
                "user@secutils.dev",
                "handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
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

        let user_by_id = db.get_user(1.try_into()?).await?.unwrap();
        let user_by_email = db.get_user_by_email("dev@secutils.dev").await?.unwrap();
        assert_eq!(user_by_id.id, user_by_email.id);
        assert_debug_snapshot!(user_by_email, @r###"
        User {
            id: UserId(
                1,
            ),
            email: "dev@secutils.dev",
            handle: "dev-handle",
            credentials: StoredCredentials {
                password_hash: Some(
                    "hash",
                ),
                passkey: None,
            },
            created: 2000-01-01 10:00:00.0 +00:00:00,
            activated: true,
            subscription: UserSubscription {
                tier: Ultimate,
                started_at: 2000-01-01 10:00:01.0 +00:00:00,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            },
        }
        "###);

        let user_by_id = db.get_user(2.try_into()?).await?.unwrap();
        let user_by_email = db.get_user_by_email("prod@secutils.dev").await?.unwrap();
        assert_eq!(user_by_id.id, user_by_email.id);
        assert_debug_snapshot!(user_by_email, @r###"
        User {
            id: UserId(
                2,
            ),
            email: "prod@secutils.dev",
            handle: "prod-handle",
            credentials: StoredCredentials {
                password_hash: Some(
                    "hash_prod",
                ),
                passkey: None,
            },
            created: 2010-01-01 10:00:00.0 +00:00:00,
            activated: false,
            subscription: UserSubscription {
                tier: Standard,
                started_at: 2010-01-01 10:00:00.0 +00:00:00,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            },
        }
        "###);

        let user_by_id = db.get_user(3.try_into()?).await?.unwrap();
        let user_by_email = db.get_user_by_email("user@secutils.dev").await?.unwrap();
        assert_eq!(user_by_id.id, user_by_email.id);
        assert_debug_snapshot!(user_by_email, @r###"
        User {
            id: UserId(
                3,
            ),
            email: "user@secutils.dev",
            handle: "handle",
            credentials: StoredCredentials {
                password_hash: Some(
                    "hash",
                ),
                passkey: None,
            },
            created: 2000-01-01 10:00:00.0 +00:00:00,
            activated: false,
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

        assert!(db
            .get_user_by_email("unknown@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn ignores_email_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::default(),
            "DeV@secutils.dev",
            "DeV-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
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
        .set_activated()
        .build();
        let db = mock_db().await?;
        let id = db.upsert_user(&user).await?;

        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?,  @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "DeV@secutils.dev",
                handle: "DeV-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
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
            id
        );
        assert_eq!(
            db.get_user_by_email("DeV@secutils.dev").await?.unwrap().id,
            id
        );

        Ok(())
    }

    #[tokio::test]
    async fn ignores_handle_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::default(),
            "DeV@secutils.dev",
            "DeV-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_activated()
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Professional,
            started_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        })
        .build();
        let db = mock_db().await?;
        let id = db.upsert_user(&user).await?;

        assert_debug_snapshot!(db.get_user_by_handle("dev-handle").await?,  @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "DeV@secutils.dev",
                handle: "DeV-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
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
        assert_eq!(db.get_user_by_handle("DEV-handle").await?.unwrap().id, id);
        assert_eq!(db.get_user_by_handle("DeV-handle").await?.unwrap().id, id);

        Ok(())
    }

    #[tokio::test]
    async fn can_insert_user() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let user_id = db
            .insert_user(
                &MockUserBuilder::new(
                    UserId::default(),
                    "dev@secutils.dev",
                    "dev-handle",
                    StoredCredentials {
                        password_hash: Some("hash".to_string()),
                        ..Default::default()
                    },
                    // January 1, 2000 11:00:00
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .set_activated()
                .build(),
            )
            .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
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

        let conflict_error = db
            .insert_user(
                &MockUserBuilder::new(
                    100.try_into()?,
                    "DEV@secutils.dev",
                    "DEV-handle",
                    StoredCredentials {
                        password_hash: Some("new-hash".to_string()),
                        ..Default::default()
                    },
                    // January 1, 2000 11:00:00
                    OffsetDateTime::from_unix_timestamp(1262340000)?,
                )
                .build(),
            )
            .await;
        assert_debug_snapshot!(conflict_error, @r###"
        Err(
            Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: users.handle",
                },
            ),
        )
        "###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            user_id
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_update_user() -> anyhow::Result<()> {
        let db = mock_db().await?;

        db.upsert_user(
            &MockUserBuilder::new(
                UserId::default(),
                "dev@secutils.dev",
                "dev-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_activated()
            .build(),
        )
        .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
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
                100.try_into()?,
                "DEV@secutils.dev",
                "DEV-handle",
                StoredCredentials {
                    password_hash: Some("new-hash".to_string()),
                    ..Default::default()
                },
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
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "DEV-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "new-hash",
                    ),
                    passkey: None,
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activated: false,
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

    #[tokio::test]
    async fn can_remove_user() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());

        let user_dev = MockUserBuilder::new(
            UserId::default(),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_activated()
        .build();
        let user_prod = MockUserBuilder::new(
            UserId::default(),
            "prod@secutils.dev",
            "prod-handle",
            StoredCredentials {
                password_hash: Some("hash_prod".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        let dev_user_id = db.upsert_user(&user_dev).await?;
        let prod_user_id = db.upsert_user(&user_prod).await?;

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            dev_user_id
        );
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            prod_user_id
        );

        assert_eq!(
            db.remove_user_by_email("dev@secutils.dev").await?.unwrap(),
            dev_user_id
        );
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.remove_user_by_email("dev@secutils.dev").await?.is_none());
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            prod_user_id
        );

        assert_eq!(
            db.remove_user_by_email("prod@secutils.dev").await?.unwrap(),
            prod_user_id
        );
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());
        assert!(db
            .remove_user_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_manipulate_user_data() -> anyhow::Result<()> {
        let db = mock_db().await?;
        let user = MockUserBuilder::new(
            1.try_into()?,
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .set_activated()
        .build();

        // No user and no data yet.
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        db.upsert_user(&user).await?;

        // Nodata yet.
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        // Insert data.
        db.upsert_user_data(
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                user.id,
                "data",
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                user.id,
                "data".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Update data.
        db.upsert_user_data(
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                user.id,
                "data-new",
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                user.id,
                "data-new".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Remove data.
        db.remove_user_data(user.id, PublicUserDataNamespace::UserSettings)
            .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_remove_old_user_data() -> anyhow::Result<()> {
        let db = mock_db().await?;

        // Create test users
        let users = vec![
            MockUserBuilder::new(
                1.try_into()?,
                "dev@secutils.dev",
                "dev-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                OffsetDateTime::now_utc(),
            )
            .set_activated()
            .build(),
            MockUserBuilder::new(
                2.try_into()?,
                "prod@secutils.dev",
                "prod-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                OffsetDateTime::now_utc(),
            )
            .set_activated()
            .build(),
        ];
        for user in users {
            db.upsert_user(&user).await?;
        }

        // Insert data for both users.
        db.upsert_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            // January 1, 2000 11:00:00
            UserData::new(
                1.try_into()?,
                "data-1",
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        db.upsert_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            // January 1, 2010 11:00:00
            UserData::new(
                2.try_into()?,
                "data-2",
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            ),
        )
        .await?;

        // Check that data exists.
        assert_debug_snapshot!(db.get_user_data::<String>(1.try_into()?, InternalUserDataNamespace::AccountActivationToken)
                .await?, @r###"
        Some(
            UserData {
                user_id: UserId(
                    1,
                ),
                key: None,
                value: "data-1",
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_data::<String>(2.try_into()?, InternalUserDataNamespace::AccountActivationToken)
                .await?, @r###"
        Some(
            UserData {
                user_id: UserId(
                    2,
                ),
                key: None,
                value: "data-2",
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);

        // Run cleanup
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(946720800)?.sub(Duration::from_secs(60)),
        )
        .await?;

        // All data should still stay.
        assert!(db
            .get_user_data::<String>(
                1.try_into()?,
                InternalUserDataNamespace::AccountActivationToken
            )
            .await?
            .is_some());
        assert!(db
            .get_user_data::<String>(
                2.try_into()?,
                InternalUserDataNamespace::AccountActivationToken
            )
            .await?
            .is_some());

        // Run cleanup with another `since`.
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(946720800)?.add(Duration::from_secs(60)),
        )
        .await?;
        assert!(db
            .get_user_data::<String>(
                1.try_into()?,
                InternalUserDataNamespace::AccountActivationToken
            )
            .await?
            .is_none());
        assert!(db
            .get_user_data::<String>(
                2.try_into()?,
                InternalUserDataNamespace::AccountActivationToken
            )
            .await?
            .is_some());

        // Run cleanup with another `since`.
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(1262340000)?.add(Duration::from_secs(60)),
        )
        .await?;
        assert!(db
            .get_user_data::<String>(
                2.try_into()?,
                InternalUserDataNamespace::AccountActivationToken
            )
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_add_and_retrieve_user_shares() -> anyhow::Result<()> {
        let user_shares = vec![
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: 1.try_into()?,
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000002")),
                user_id: 2.try_into()?,
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000002"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let db = mock_db().await?;
        db.insert_user(mock_user_with_id(1)?).await?;
        db.insert_user(mock_user_with_id(2)?).await?;

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

    #[tokio::test]
    async fn can_retrieve_user_shares_by_resource() -> anyhow::Result<()> {
        let user_shares = [
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: 1.try_into()?,
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000002")),
                user_id: 2.try_into()?,
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000002"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let db = mock_db().await?;
        db.insert_user(mock_user_with_id(1)?).await?;
        db.insert_user(mock_user_with_id(2)?).await?;

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

        assert!(db
            .get_user_share_by_resource(3.try_into()?, &user_shares[0].resource)
            .await?
            .is_none());
        assert!(db
            .get_user_share_by_resource(
                user_shares[0].user_id,
                &SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000003"
                ))
            )
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_remove_user_shares() -> anyhow::Result<()> {
        let user_shares = vec![
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: 1.try_into()?,
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000002")),
                user_id: 2.try_into()?,
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000002"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let db = mock_db().await?;
        db.insert_user(mock_user_with_id(1)?).await?;
        db.insert_user(mock_user_with_id(2)?).await?;

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
