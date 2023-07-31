mod raw_user;
mod raw_user_data;
mod raw_user_to_upsert;

use self::{raw_user::RawUser, raw_user_data::RawUserData, raw_user_to_upsert::RawUserToUpsert};
use crate::{
    datastore::PrimaryDb,
    users::{User, UserData, UserDataKey, UserId},
};
use anyhow::bail;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{query, query_as, query_scalar};
use time::OffsetDateTime;

/// Extends primary DB with the user management-related methods.
impl PrimaryDb {
    /// Retrieves user from the `Users` table using user email.
    pub async fn get_user_by_email<T: AsRef<str>>(&self, email: T) -> anyhow::Result<Option<User>> {
        let email = email.as_ref();
        query_as!(
            RawUser,
            r#"
SELECT id, email, handle, credentials, created, roles, activated
FROM users
WHERE email = ?1
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
SELECT id, email, handle, credentials, created, roles, activated
FROM users
WHERE handle = ?1
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

        let user_id: i64 = query_scalar!(
            r#"
INSERT INTO users (email, handle, credentials, created, roles, activated)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.credentials,
            raw_user.created,
            raw_user.roles,
            raw_user.activated
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(UserId(user_id))
    }

    /// Inserts or updates user in the `Users` table.
    pub async fn upsert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        let raw_user = RawUserToUpsert::try_from(user.as_ref())?;

        let user_id: i64 = query_scalar!(r#"
INSERT INTO users (email, handle, credentials, created, roles, activated)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
ON CONFLICT(email) DO UPDATE SET handle=excluded.handle, credentials=excluded.credentials, created=excluded.created, roles=excluded.roles, activated=excluded.activated
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.credentials,
            raw_user.created,
            raw_user.roles,
            raw_user.activated
        )
            .fetch_one(&self.pool)
            .await?;

        Ok(UserId(user_id))
    }

    /// Removes user with the specified email from the `Users` table.
    pub async fn remove_user_by_email<T: AsRef<str>>(
        &self,
        email: T,
    ) -> anyhow::Result<Option<User>> {
        let email = email.as_ref();
        query_as!(
            RawUser,
            r#"
DELETE FROM users
WHERE email = ?1
RETURNING id as "id!", email as "email!", handle as "handle!", credentials as "credentials!", created as "created!", roles, activated
            "#,
            email
        )
            .fetch_optional(&self.pool)
            .await?
            .map(User::try_from)
            .transpose()
    }

    /// Retrieves user data from the `UserData` table using user id and data key.
    pub async fn get_user_data<R: DeserializeOwned>(
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
SELECT value, timestamp
FROM user_data
WHERE user_id = ?1 AND namespace = ?2 AND key = ?3
                "#,
            user_id.0,
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
        user_id: UserId,
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
            user_id.0,
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
            user_id.0,
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
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::StoredCredentials,
        tests::{mock_db, MockUserBuilder},
        users::{InternalUserDataNamespace, PublicUserDataNamespace, UserData, UserId},
    };
    use insta::assert_debug_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[actix_rt::test]
    async fn can_add_and_retrieve_users() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_user_by_email("some-id").await?.is_none());

        let users = vec![
            MockUserBuilder::new(
                UserId::empty(),
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
                UserId::empty(),
                "prod@secutils.dev",
                "prod-handle",
                StoredCredentials {
                    password_hash: Some("hash_prod".to_string()),
                    ..Default::default()
                },
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .add_role("admin")
            .build(),
            MockUserBuilder::new(
                UserId::empty(),
                "user@secutils.dev",
                "handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .add_role("Power-User")
            .build(),
        ];
        for user in users {
            db.upsert_user(&user).await?;
        }

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
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_by_email("prod@secutils.dev").await?, @r###"
        Some(
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
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activated: false,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_by_email("user@secutils.dev").await?, @r###"
        Some(
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
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: false,
            },
        )
        "###);
        assert!(db
            .get_user_by_email("unknown@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn ignores_email_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::empty(),
            "DeV@secutils.dev",
            "DeV-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .add_role("Power-User")
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
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
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

    #[actix_rt::test]
    async fn ignores_handle_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::empty(),
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
        .add_role("Power-User")
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
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);
        assert_eq!(db.get_user_by_handle("DEV-handle").await?.unwrap().id, id);
        assert_eq!(db.get_user_by_handle("DeV-handle").await?.unwrap().id, id);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_insert_user() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let user_id = db
            .insert_user(
                &MockUserBuilder::new(
                    UserId::empty(),
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
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);

        let conflict_error = db
            .insert_user(
                &MockUserBuilder::new(
                    UserId(100),
                    "DEV@secutils.dev",
                    "DEV-handle",
                    StoredCredentials {
                        password_hash: Some("new-hash".to_string()),
                        ..Default::default()
                    },
                    // January 1, 2000 11:00:00
                    OffsetDateTime::from_unix_timestamp(1262340000)?,
                )
                .add_role("admin")
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

    #[actix_rt::test]
    async fn can_update_user() -> anyhow::Result<()> {
        let db = mock_db().await?;

        db.upsert_user(
            &MockUserBuilder::new(
                UserId::empty(),
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
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);

        db.upsert_user(
            &MockUserBuilder::new(
                UserId(100),
                "DEV@secutils.dev",
                "DEV-handle",
                StoredCredentials {
                    password_hash: Some("new-hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .add_role("admin")
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
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activated: false,
            },
        )
        "###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            db.get_user_by_email("DEV@secutils.dev").await?.unwrap().id
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_user() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());

        let user_dev = MockUserBuilder::new(
            UserId::empty(),
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
            UserId::empty(),
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
            db.remove_user_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .id,
            dev_user_id
        );
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.remove_user_by_email("dev@secutils.dev").await?.is_none());
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            prod_user_id
        );

        assert_eq!(
            db.remove_user_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .id,
            prod_user_id
        );
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());
        assert!(db
            .remove_user_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_manipulate_user_data() -> anyhow::Result<()> {
        let db = mock_db().await?;
        let user = MockUserBuilder::new(
            UserId(1),
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
            user.id,
            PublicUserDataNamespace::UserSettings,
            UserData::new("data", OffsetDateTime::from_unix_timestamp(946720800)?),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                "data".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Update data.
        db.upsert_user_data(
            user.id,
            PublicUserDataNamespace::UserSettings,
            UserData::new("data-new", OffsetDateTime::from_unix_timestamp(946720800)?),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
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

    #[actix_rt::test]
    async fn can_remove_old_user_data() -> anyhow::Result<()> {
        let db = mock_db().await?;

        // Create test users
        let users = vec![
            MockUserBuilder::new(
                UserId(1),
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
                UserId(2),
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
            UserId(1),
            InternalUserDataNamespace::AccountActivationToken,
            // January 1, 2000 11:00:00
            UserData::new("data-1", OffsetDateTime::from_unix_timestamp(946720800)?),
        )
        .await?;
        db.upsert_user_data(
            UserId(2),
            InternalUserDataNamespace::AccountActivationToken,
            // January 1, 2010 11:00:00
            UserData::new("data-2", OffsetDateTime::from_unix_timestamp(1262340000)?),
        )
        .await?;

        // Check that data exists.
        assert_debug_snapshot!(db.get_user_data::<String>(UserId(1), InternalUserDataNamespace::AccountActivationToken)
                .await?, @r###"
        Some(
            UserData {
                value: "data-1",
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
                .await?, @r###"
        Some(
            UserData {
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
            .get_user_data::<String>(UserId(1), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_some());
        assert!(db
            .get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_some());

        // Run cleanup with another `since`.
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(946720800)?.add(Duration::from_secs(60)),
        )
        .await?;
        assert!(db
            .get_user_data::<String>(UserId(1), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_none());
        assert!(db
            .get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_some());

        // Run cleanup with another `since`.
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(1262340000)?.add(Duration::from_secs(60)),
        )
        .await?;
        assert!(db
            .get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_none());

        Ok(())
    }
}