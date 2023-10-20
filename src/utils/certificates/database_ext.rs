mod raw_private_key;

use self::raw_private_key::RawPrivateKey;
use crate::{database::Database, error::Error as SecutilsError, users::UserId, utils::PrivateKey};
use anyhow::{anyhow, bail};
use sqlx::{error::ErrorKind as SqlxErrorKind, query, query_as, Pool, Sqlite};
use uuid::Uuid;

/// A database extension for the certificate utility-related operations.
pub struct CertificatesDatabaseExt<'pool> {
    pool: &'pool Pool<Sqlite>,
}

impl<'pool> CertificatesDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Retrieves private key for the specified user with the specified name.
    pub async fn get_private_key(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<PrivateKey>> {
        let id = id.as_ref();
        query_as!(
            RawPrivateKey,
            r#"
SELECT id, name, alg, pkcs8, encrypted, created_at
FROM user_data_certificates_private_keys
WHERE user_id = ?1 AND id = ?2
                "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(PrivateKey::try_from)
        .transpose()
    }

    /// Inserts private key.
    pub async fn insert_private_key(
        &self,
        user_id: UserId,
        private_key: &PrivateKey,
    ) -> anyhow::Result<()> {
        let raw_private_key = RawPrivateKey::try_from(private_key)?;
        let result = query!(
            r#"
INSERT INTO user_data_certificates_private_keys (user_id, id, name, alg, pkcs8, encrypted, created_at)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 )
        "#,
            *user_id,
            raw_private_key.id,
            raw_private_key.name,
            raw_private_key.alg,
            raw_private_key.pkcs8,
            raw_private_key.encrypted,
            raw_private_key.created_at
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
                    "Private key ('{}') already exists.",
                    private_key.name
                )))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create private key ('{}') due to unknown reason.",
                    private_key.name
                )))
            });
        }

        Ok(())
    }

    /// Updates private key (only `name` and `pkcs8` content can be updated due to password change).
    pub async fn update_private_key(
        &self,
        user_id: UserId,
        private_key: &PrivateKey,
    ) -> anyhow::Result<()> {
        let raw_private_key = RawPrivateKey::try_from(private_key)?;
        let result = query!(
            r#"
UPDATE user_data_certificates_private_keys
SET name = ?3, pkcs8 = ?4, encrypted = ?5
WHERE user_id = ?1 AND id = ?2
        "#,
            *user_id,
            raw_private_key.id,
            raw_private_key.name,
            raw_private_key.pkcs8,
            raw_private_key.encrypted
        )
        .execute(self.pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "A private key ('{}') doesn't exist.",
                        private_key.name
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
                        "Private key ('{}') already exists.",
                        private_key.name
                    )))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update private key ('{}') due to unknown reason.",
                        private_key.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes private key for the specified user with the specified name.
    pub async fn remove_private_key(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        let id = id.as_ref();
        query!(
            r#"
DELETE FROM user_data_certificates_private_keys
WHERE user_id = ?1 AND id = ?2
                "#,
            *user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all private keys for the specified user.
    pub async fn get_private_keys(&self, user_id: UserId) -> anyhow::Result<Vec<PrivateKey>> {
        // When returning data about all private keys, we don't return the pkcs8 data itself since
        // it's supposed to be retrieved only one by one.
        let raw_private_keys = query_as!(
            RawPrivateKey,
            r#"
SELECT id, name, alg, x'' as "pkcs8!", encrypted, created_at
FROM user_data_certificates_private_keys
WHERE user_id = ?1
ORDER BY created_at
                "#,
            *user_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut private_keys = vec![];
        for raw_private_key in raw_private_keys {
            private_keys.push(PrivateKey::try_from(raw_private_key)?);
        }

        Ok(private_keys)
    }
}

impl Database {
    /// Returns a database extension for the certificate utility-related operations.
    pub fn certificates(&self) -> CertificatesDatabaseExt {
        CertificatesDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        tests::{mock_db, mock_user},
        utils::{PrivateKey, PrivateKeyAlgorithm, PrivateKeySize},
    };
    use actix_web::ResponseError;
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[actix_rt::test]
    async fn can_add_and_retrieve_private_keys() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut private_keys = vec![
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "pk-name-2".to_string(),
                alg: PrivateKeyAlgorithm::Dsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![4, 5, 6],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
            },
        ];

        for private_key in private_keys.iter() {
            db.certificates()
                .insert_private_key(user.id, private_key)
                .await?;
        }

        let private_key = db
            .certificates()
            .get_private_key(user.id, private_keys[0].id)
            .await?
            .unwrap();
        assert_eq!(private_key, private_keys.remove(0));

        let private_key = db
            .certificates()
            .get_private_key(user.id, private_keys[0].id)
            .await?
            .unwrap();
        assert_eq!(private_key, private_keys.remove(0));

        assert!(db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000003"))
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_handles_duplicated_private_keys_on_insert() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let private_key = PrivateKey {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "pk-name".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048,
            },
            pkcs8: vec![1, 2, 3],
            encrypted: true,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };

        db.certificates()
            .insert_private_key(user.id, &private_key)
            .await?;

        let insert_error = db
            .certificates()
            .insert_private_key(user.id, &private_key)
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(insert_error.status_code(), 400);
        assert_debug_snapshot!(
            insert_error,
            @r###"
        Error {
            context: "Private key (\'pk-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_certificates_private_keys.name, user_data_certificates_private_keys.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_private_key_content() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        db.certificates()
            .insert_private_key(
                user.id,
                &PrivateKey {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "pk-name".to_string(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size2048,
                    },
                    pkcs8: vec![1, 2, 3],
                    encrypted: true,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
            )
            .await?;

        db.certificates()
            .update_private_key(
                user.id,
                &PrivateKey {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "pk-name-new".to_string(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    },
                    pkcs8: vec![4, 5, 6],
                    encrypted: false,
                    created_at: OffsetDateTime::from_unix_timestamp(956720800)?,
                },
            )
            .await?;

        let private_key = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            private_key,
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name-new".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![4, 5, 6],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_handles_duplicated_private_keys_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let private_key_a = PrivateKey {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "pk-name-a".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048,
            },
            pkcs8: vec![1, 2, 3],
            encrypted: true,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };
        db.certificates()
            .insert_private_key(user.id, &private_key_a)
            .await?;

        let private_key_b = PrivateKey {
            id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "pk-name-b".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048,
            },
            pkcs8: vec![3, 4, 5],
            encrypted: true,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };
        db.certificates()
            .insert_private_key(user.id, &private_key_b)
            .await?;

        let update_error = db
            .certificates()
            .update_private_key(
                user.id,
                &PrivateKey {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "pk-name-a".to_string(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size2048,
                    },
                    pkcs8: vec![3, 4, 5],
                    encrypted: true,
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
            context: "Private key (\'pk-name-a\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_certificates_private_keys.name, user_data_certificates_private_keys.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_handles_non_existent_private_keys_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let update_error = db
            .certificates()
            .update_private_key(
                user.id,
                &PrivateKey {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "pk-name-a".to_string(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size2048,
                    },
                    pkcs8: vec![3, 4, 5],
                    encrypted: true,
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
            @r###""A private key ('pk-name-a') doesn't exist.""###
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_private_keys() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut private_keys = vec![
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "pk-name-2".to_string(),
                alg: PrivateKeyAlgorithm::Dsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![4, 5, 6],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
            },
        ];

        for private_key in private_keys.iter() {
            db.certificates()
                .insert_private_key(user.id, private_key)
                .await?;
        }

        let private_key = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(private_key, private_keys.remove(0));

        let private_key_2 = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(private_key_2, private_keys[0].clone());

        db.certificates()
            .remove_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let private_key = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(private_key.is_none());

        let private_key = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(private_key, private_keys.remove(0));

        db.certificates()
            .remove_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let private_key = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(private_key.is_none());

        let private_key = db
            .certificates()
            .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(private_key.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_all_private_keys() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let private_keys = vec![
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "pk-name-2".to_string(),
                alg: PrivateKeyAlgorithm::Dsa {
                    key_size: PrivateKeySize::Size2048,
                },
                pkcs8: vec![4, 5, 6],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
            },
        ];

        for private_key in private_keys.iter() {
            db.certificates()
                .insert_private_key(user.id, private_key)
                .await?;
        }

        assert_eq!(
            db.certificates().get_private_keys(user.id).await?,
            private_keys
                .into_iter()
                .map(|mut private_key| {
                    private_key.pkcs8.clear();
                    private_key
                })
                .collect::<Vec<_>>()
        );

        Ok(())
    }
}
