mod raw_user;
mod raw_user_data;
mod raw_user_to_upsert;

use crate::users::{User, UserId};
use anyhow::{bail, Context};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{query, query_as, query_scalar, sqlite::SqlitePool, Pool, Sqlite};

use raw_user::RawUser;
use raw_user_data::RawUserData;
use raw_user_to_upsert::RawUserToUpsert;

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
SELECT id, email, handle, password_hash, created, roles, activation_code
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
SELECT id, email, handle, password_hash, created, roles, activation_code
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

    /// Retrieves users from the `Users` table using activation code.
    pub async fn get_users_by_activation_code<T: AsRef<str>>(
        &self,
        activation_code: T,
    ) -> anyhow::Result<Vec<User>> {
        let activation_code = activation_code.as_ref();
        query_as!(
            RawUser,
            r#"
SELECT id, email, handle, password_hash, created, roles, activation_code
FROM users
WHERE activation_code = ?1
            "#,
            activation_code
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(User::try_from)
        .collect()
    }

    /// Inserts or updates user in the `Users` index.
    pub async fn upsert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        let raw_user = RawUserToUpsert::try_from(user.as_ref())?;

        let user_id: i64 = query_scalar!(r#"
INSERT INTO users (email, handle, password_hash, created, roles, activation_code)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
ON CONFLICT(email) DO UPDATE SET handle=excluded.handle, password_hash=excluded.password_hash, created=excluded.created, roles=excluded.roles, activation_code=excluded.activation_code
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.password_hash,
            raw_user.created,
            raw_user.roles,
            raw_user.activation_code
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
        let mut conn = self.pool.acquire().await?;
        query_as!(
            RawUser,
            r#"
DELETE FROM users
WHERE email = ?1
RETURNING id as "id!", email as "email!", handle as "handle!", password_hash as "password_hash!", created as "created!", roles, activation_code
            "#,
            email
        )
        .fetch_optional(&mut conn)
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
                .with_context(|| format!("Cannot deserialize user data ({}).", user_data_key))
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
            .with_context(|| format!("Failed to serialize user data ({})", user_data_key))?;

        let mut conn = self.pool.acquire().await?;
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
        .execute(&mut conn)
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
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::PrimaryDb,
        tests::MockUserBuilder,
        users::{User, UserId},
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[actix_rt::test]
    async fn can_add_and_retrieve_users() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        assert_eq!(db.get_user_by_email("some-id").await?, None);

        let users = vec![
            MockUserBuilder::new(
                UserId::empty(),
                "dev@secutils.dev",
                "dev-handle",
                "hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
            MockUserBuilder::new(
                UserId::empty(),
                "prod@secutils.dev",
                "prod-handle",
                "hash_prod",
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_activation_code("some-code")
            .add_role("admin")
            .build(),
            MockUserBuilder::new(
                UserId::empty(),
                "user@secutils.dev",
                "handle",
                "hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_activation_code("some-user-code")
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
                password_hash: "hash",
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activation_code: None,
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
                password_hash: "hash_prod",
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activation_code: Some(
                    "some-code",
                ),
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
                password_hash: "hash",
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activation_code: Some(
                    "some-user-code",
                ),
            },
        )
        "###);
        assert_eq!(db.get_user_by_email("unknown@secutils.dev").await?, None);

        Ok(())
    }

    #[actix_rt::test]
    async fn ignores_email_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::empty(),
            "DeV@secutils.dev",
            "DeV-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .add_role("user")
        .add_role("Power-User")
        .build();
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        let id = db.upsert_user(&user).await?;

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?,
            Some(User { id, ..user.clone() })
        );
        assert_eq!(
            db.get_user_by_email("DEV@secutils.dev").await?,
            Some(User { id, ..user.clone() })
        );
        assert_eq!(
            db.get_user_by_email("DeV@secutils.dev").await?,
            Some(User { id, ..user })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn ignores_handle_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::empty(),
            "DeV@secutils.dev",
            "DeV-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .add_role("user")
        .add_role("Power-User")
        .build();
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        let id = db.upsert_user(&user).await?;

        assert_eq!(
            db.get_user_by_handle("dev-handle").await?,
            Some(User { id, ..user.clone() })
        );
        assert_eq!(
            db.get_user_by_handle("DEV-handle").await?,
            Some(User { id, ..user.clone() })
        );
        assert_eq!(
            db.get_user_by_handle("DeV-handle").await?,
            Some(User { id, ..user })
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
                "hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
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
                password_hash: "hash",
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activation_code: None,
            },
        )
        "###);

        db.upsert_user(
            &MockUserBuilder::new(
                UserId(100),
                "DEV@secutils.dev",
                "DEV-handle",
                "new-hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_activation_code("some-code")
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
                password_hash: "new-hash",
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activation_code: Some(
                    "some-code",
                ),
            },
        )
        "###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?,
            db.get_user_by_email("DEV@secutils.dev").await?
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_user() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        assert_eq!(db.get_user_by_email("dev@secutils.dev").await?, None);
        assert_eq!(db.get_user_by_email("prod@secutils.dev").await?, None);

        let user_dev = MockUserBuilder::new(
            UserId::empty(),
            "dev@secutils.dev",
            "dev-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .build();
        let user_prod = MockUserBuilder::new(
            UserId::empty(),
            "prod@secutils.dev",
            "prod-handle",
            "hash_prod",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activation_code("some-code")
        .build();

        let dev_user_id = db.upsert_user(&user_dev).await?;
        let prod_user_id = db.upsert_user(&user_prod).await?;

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?,
            Some(User {
                id: dev_user_id,
                ..user_dev.clone()
            })
        );
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?,
            Some(User {
                id: prod_user_id,
                ..user_prod.clone()
            })
        );

        assert_eq!(
            db.remove_user_by_email("dev@secutils.dev").await?,
            Some(User {
                id: dev_user_id,
                ..user_dev
            })
        );
        assert_eq!(db.get_user_by_email("dev@secutils.dev").await?, None);
        assert_eq!(db.remove_user_by_email("dev@secutils.dev").await?, None);
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?,
            Some(User {
                id: prod_user_id,
                ..user_prod.clone()
            })
        );

        assert_eq!(
            db.remove_user_by_email("prod@secutils.dev").await?,
            Some(User {
                id: prod_user_id,
                ..user_prod
            })
        );
        assert_eq!(db.get_user_by_email("prod@secutils.dev").await?, None);
        assert_eq!(db.remove_user_by_email("prod@secutils.dev").await?, None);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_search_users() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        let user_dev = MockUserBuilder::new(
            UserId::empty(),
            "dev@secutils.dev",
            "dev-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_activation_code("some-code")
        .build();
        let user_prod = MockUserBuilder::new(
            UserId::empty(),
            "prod@secutils.dev",
            "prod-handle",
            "hash_prod",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activation_code("OTHER-code")
        .build();

        let dev_user_id = db.upsert_user(&user_dev).await?;
        let prod_user_id = db.upsert_user(&user_prod).await?;

        assert_eq!(
            db.get_users_by_activation_code("some-code").await?,
            vec![User {
                id: dev_user_id,
                ..user_dev.clone()
            }]
        );
        assert_eq!(
            db.get_users_by_activation_code("SOME-code").await?,
            vec![User {
                id: dev_user_id,
                ..user_dev
            }]
        );

        assert_eq!(
            db.get_users_by_activation_code("other-code").await?,
            vec![User {
                id: prod_user_id,
                ..user_prod.clone()
            }]
        );
        assert_eq!(
            db.get_users_by_activation_code("OTHER-code").await?,
            vec![User {
                id: prod_user_id,
                ..user_prod
            }]
        );

        assert_eq!(
            db.get_users_by_activation_code("unknown-code").await?,
            vec![]
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_manipulate_user_data() -> anyhow::Result<()> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        let user = MockUserBuilder::new(
            UserId(1),
            "dev@secutils.dev",
            "dev-handle",
            "hash",
            OffsetDateTime::now_utc(),
        )
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
}
