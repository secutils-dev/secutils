mod raw_user;
mod raw_user_data;
mod raw_user_to_upsert;
mod raw_user_webauthn_session;
mod raw_util;

use crate::{
    authentication::WebAuthnSession,
    users::{User, UserId},
    utils::Util,
};
use anyhow::{bail, Context};
use raw_user::RawUser;
use raw_user_data::RawUserData;
use raw_user_to_upsert::RawUserToUpsert;
use raw_user_webauthn_session::RawUserWebAuthnSession;
use raw_util::RawUtil;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{query, query_as, query_scalar, sqlite::SqlitePool, Pool, Sqlite};
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct PrimaryDb {
    pool: Pool<Sqlite>,
}

impl PrimaryDb {
    /// Opens primary DB "connection".
    pub async fn open<I: FnOnce() -> anyhow::Result<String>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(&initializer()?).await?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .with_context(|| "Failed to migrate database")?;

        Ok(PrimaryDb { pool })
    }

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
        user_data_key: &str,
    ) -> anyhow::Result<Option<R>> {
        query_as!(
            RawUserData,
            r#"
SELECT data_value
FROM user_data
WHERE user_id = ?1 AND data_key = ?2
                "#,
            user_id.0,
            user_data_key
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|raw_user_data| {
            serde_json::from_slice::<R>(&raw_user_data.data_value)
                .with_context(|| format!("Cannot deserialize user data ({user_data_key})."))
        })
        .transpose()
    }

    /// Sets user data in the `UserData` table using user id and data key.
    pub async fn upsert_user_data<R: Serialize>(
        &self,
        user_id: UserId,
        user_data_key: &str,
        data_value: R,
    ) -> anyhow::Result<()> {
        let user_data_value = serde_json::ser::to_vec(&data_value)
            .with_context(|| format!("Failed to serialize user data ({user_data_key})."))?;
        query!(
            r#"
INSERT INTO user_data (user_id, data_key, data_value)
VALUES ( ?1, ?2, ?3 )
ON CONFLICT(user_id, data_key) DO UPDATE SET data_value=excluded.data_value
        "#,
            user_id.0,
            user_data_key,
            user_data_value
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user data from the `UserData` table using user id and data key.
    pub async fn remove_user_data(
        &self,
        user_id: UserId,
        user_data_key: &str,
    ) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_data
WHERE user_id = ?1 AND data_key = ?2
            "#,
            user_id.0,
            user_data_key
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves user's WebAuthn session from the `UserWebAuthnSessions` table using user email.
    pub async fn get_user_webauthn_session_by_email<E: AsRef<str>>(
        &self,
        email: E,
    ) -> anyhow::Result<Option<WebAuthnSession>> {
        let email = email.as_ref();
        query_as!(
            RawUserWebAuthnSession,
            r#"
SELECT email, session_value, timestamp
FROM user_webauthn_sessions
WHERE email = ?1
                "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .map(WebAuthnSession::try_from)
        .transpose()
    }

    /// Sets user's WebAuthn session in the `UserWebAuthnSessions` table.
    pub async fn upsert_user_webauthn_session(
        &self,
        session: &WebAuthnSession,
    ) -> anyhow::Result<()> {
        let raw_session_value = serde_json::ser::to_vec(&session.value).with_context(|| {
            format!(
                "Failed to serialize user WebAuthn session ({}).",
                session.email
            )
        })?;
        let raw_session_timestamp = session.timestamp.unix_timestamp();

        query!(
            r#"
INSERT INTO user_webauthn_sessions (email, session_value, timestamp)
VALUES (?1, ?2, ?3)
ON CONFLICT(email) DO UPDATE SET session_value=excluded.session_value, timestamp=excluded.timestamp
        "#,
            session.email,
            raw_session_value,
            raw_session_timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user's WebAuthn session from the `UserWebAuthnSessions` table using user email.
    pub async fn remove_user_webauthn_session_by_email(&self, email: &str) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_webauthn_sessions
WHERE email = ?1
            "#,
            email
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes WebAuthn sessions that are older than specified timestamp.
    pub async fn remove_user_webauthn_sessions(&self, since: OffsetDateTime) -> anyhow::Result<()> {
        let since = since.unix_timestamp();
        query!(
            r#"
DELETE FROM user_webauthn_sessions
WHERE timestamp <= ?1
            "#,
            since
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all utils from the `Utils` table.
    pub async fn get_utils(&self) -> anyhow::Result<Vec<Util>> {
        let mut root_utils = query_as!(
            RawUtil,
            r#"
SELECT id, handle, name, keywords, parent_id
FROM utils
ORDER BY parent_id, id
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        // Utilities are sorted by the parent_id meaning that all root utilities are returned first.
        let child_utils = if let Some(position) = root_utils
            .iter()
            .position(|raw_util| raw_util.parent_id.is_some())
        {
            root_utils.split_off(position)
        } else {
            return root_utils.into_iter().map(Util::try_from).collect();
        };

        let mut parent_children_map = HashMap::<_, Vec<_>>::new();
        for util in child_utils {
            if let Some(parent_id) = util.parent_id {
                parent_children_map.entry(parent_id).or_default().push(util);
            } else {
                bail!("Child utility does not have a parent id.");
            }
        }

        root_utils
            .into_iter()
            .map(|root_util| Self::build_util_tree(root_util, &mut parent_children_map))
            .collect()
    }

    fn build_util_tree(
        raw_util: RawUtil,
        parent_children_map: &mut HashMap<i64, Vec<RawUtil>>,
    ) -> anyhow::Result<Util> {
        let utils = if let Some(mut children) = parent_children_map.remove(&raw_util.id) {
            Some(
                children
                    .drain(..)
                    .map(|util| Self::build_util_tree(util, parent_children_map))
                    .collect::<anyhow::Result<_>>()?,
            )
        } else {
            None
        };

        Util::try_from(raw_util).map(|util| Util { utils, ..util })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::{StoredCredentials, WebAuthnSession, WebAuthnSessionValue},
        datastore::PrimaryDb,
        tests::{
            webauthn::{SERIALIZED_AUTHENTICATION_STATE, SERIALIZED_REGISTRATION_STATE},
            MockUserBuilder,
        },
        users::UserId,
    };
    use insta::assert_debug_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[actix_rt::test]
    async fn can_add_and_retrieve_users() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
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
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
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
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
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
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;

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
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;

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
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
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
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
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
        assert_eq!(db.get_user_data::<String>(user.id, "data-key").await?, None);

        db.upsert_user(&user).await?;

        // Nodata yet.
        assert_eq!(db.get_user_data::<String>(user.id, "data-key").await?, None);

        // Insert data.
        db.upsert_user_data(user.id, "data-key", "data").await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, "data-key").await?,
            Some("data".to_string())
        );

        // Update data.
        db.upsert_user_data(user.id, "data-key", "data-new").await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, "data-key").await?,
            Some("data-new".to_string())
        );

        // Remove data.
        db.remove_user_data(user.id, "data-key").await?;
        assert_eq!(db.get_user_data::<String>(user.id, "data-key").await?, None);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_add_and_retrieve_webauthn_sessions() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        assert!(db
            .get_user_webauthn_session_by_email("some-id")
            .await?
            .is_none());

        let sessions = vec![
            WebAuthnSession {
                email: "dev@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebAuthnSession {
                email: "prod@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2010 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            },
        ];
        for session in sessions {
            db.upsert_user_webauthn_session(&session).await?;
        }

        assert_debug_snapshot!(db.get_user_webauthn_session_by_email("dev@secutils.dev").await?, @r###"
        Some(
            WebAuthnSession {
                email: "dev@secutils.dev",
                value: RegistrationState(
                    PasskeyRegistration {
                        rs: RegistrationState {
                            policy: Preferred,
                            exclude_credentials: [],
                            challenge: Base64UrlSafeData(
                                [
                                    223,
                                    161,
                                    90,
                                    219,
                                    14,
                                    74,
                                    186,
                                    255,
                                    52,
                                    157,
                                    60,
                                    210,
                                    28,
                                    75,
                                    219,
                                    3,
                                    154,
                                    213,
                                    19,
                                    100,
                                    38,
                                    255,
                                    29,
                                    40,
                                    142,
                                    55,
                                    15,
                                    45,
                                    135,
                                    129,
                                    245,
                                    18,
                                ],
                            ),
                            credential_algorithms: [
                                ES256,
                                RS256,
                            ],
                            require_resident_key: false,
                            authenticator_attachment: None,
                            extensions: RequestRegistrationExtensions {
                                cred_protect: None,
                                uvm: Some(
                                    true,
                                ),
                                cred_props: Some(
                                    true,
                                ),
                                min_pin_length: None,
                                hmac_create_secret: None,
                            },
                            experimental_allow_passkeys: true,
                        },
                    },
                ),
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_webauthn_session_by_email("prod@secutils.dev").await?, @r###"
        Some(
            WebAuthnSession {
                email: "prod@secutils.dev",
                value: RegistrationState(
                    PasskeyRegistration {
                        rs: RegistrationState {
                            policy: Preferred,
                            exclude_credentials: [],
                            challenge: Base64UrlSafeData(
                                [
                                    223,
                                    161,
                                    90,
                                    219,
                                    14,
                                    74,
                                    186,
                                    255,
                                    52,
                                    157,
                                    60,
                                    210,
                                    28,
                                    75,
                                    219,
                                    3,
                                    154,
                                    213,
                                    19,
                                    100,
                                    38,
                                    255,
                                    29,
                                    40,
                                    142,
                                    55,
                                    15,
                                    45,
                                    135,
                                    129,
                                    245,
                                    18,
                                ],
                            ),
                            credential_algorithms: [
                                ES256,
                                RS256,
                            ],
                            require_resident_key: false,
                            authenticator_attachment: None,
                            extensions: RequestRegistrationExtensions {
                                cred_protect: None,
                                uvm: Some(
                                    true,
                                ),
                                cred_props: Some(
                                    true,
                                ),
                                min_pin_length: None,
                                hmac_create_secret: None,
                            },
                            experimental_allow_passkeys: true,
                        },
                    },
                ),
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
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
    async fn ignores_email_case_for_webauthn_sessions() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;

        db.upsert_user_webauthn_session(&WebAuthnSession {
            email: "dev@secutils.dev".to_string(),
            value: serde_json::from_str(&format!(
                "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
            ))?,
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
        .await?;

        assert_eq!(
            db.get_user_webauthn_session_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );
        assert_eq!(
            db.get_user_webauthn_session_by_email("DeV@secUtils.dEv")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_webauthn_sessions() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;

        db.upsert_user_webauthn_session(&WebAuthnSession {
            email: "dev@secutils.dev".to_string(),
            value: serde_json::from_str(&format!(
                "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
            ))?,
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
        .await?;

        let session = db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .unwrap();
        assert_eq!(session.email, "dev@secutils.dev");
        assert_eq!(
            session.timestamp,
            OffsetDateTime::from_unix_timestamp(946720800)?
        );
        assert!(matches!(
            session.value,
            WebAuthnSessionValue::RegistrationState(_)
        ));

        db.upsert_user_webauthn_session(&WebAuthnSession {
            email: "dev@secutils.dev".to_string(),
            value: serde_json::from_str(&format!(
                "{{\"AuthenticationState\":{SERIALIZED_AUTHENTICATION_STATE}}}"
            ))?,
            // January 1, 2010 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
        })
        .await?;

        let session = db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .unwrap();
        assert_eq!(session.email, "dev@secutils.dev");
        assert_eq!(
            session.timestamp,
            OffsetDateTime::from_unix_timestamp(1262340000)?
        );
        assert!(matches!(
            session.value,
            WebAuthnSessionValue::AuthenticationState(_)
        ));

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_webauthn_session() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        assert!(db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .is_none());
        assert!(db
            .get_user_webauthn_session_by_email("prod@secutils.dev")
            .await?
            .is_none());

        let sessions = vec![
            WebAuthnSession {
                email: "dev@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebAuthnSession {
                email: "prod@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2010 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            },
        ];
        for session in sessions {
            db.upsert_user_webauthn_session(&session).await?;
        }

        db.remove_user_webauthn_session_by_email("dev@secutils.dev")
            .await?;
        assert!(db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .is_none());
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_session_by_email("PROD@secutils.dev")
            .await?;
        assert!(db
            .get_user_webauthn_session_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_old_webauthn_session() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        let sessions = vec![
            WebAuthnSession {
                email: "dev@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebAuthnSession {
                email: "prod@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2010 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            },
        ];
        for session in sessions {
            db.upsert_user_webauthn_session(&session).await?;
        }

        assert_eq!(
            db.get_user_webauthn_session_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_sessions(
            OffsetDateTime::from_unix_timestamp(946720800)?.sub(Duration::from_secs(60)),
        )
        .await?;

        assert_eq!(
            db.get_user_webauthn_session_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_sessions(
            OffsetDateTime::from_unix_timestamp(946720800)?.add(Duration::from_secs(60)),
        )
        .await?;

        assert!(db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .is_none());
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_sessions(
            OffsetDateTime::from_unix_timestamp(1262340000)?.add(Duration::from_secs(60)),
        )
        .await?;

        assert!(db
            .get_user_webauthn_session_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }
}
