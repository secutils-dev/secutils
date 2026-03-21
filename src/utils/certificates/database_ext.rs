mod raw_certificate_attributes;
mod raw_certificate_template;
mod raw_private_key;
mod raw_private_key_algorithm;

use self::raw_private_key::RawPrivateKey;
use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::UserId,
    utils::certificates::{
        CertificateTemplate, PrivateKey,
        database_ext::raw_certificate_template::RawCertificateTemplate,
    },
};
use anyhow::{anyhow, bail};
use sqlx::{Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use uuid::Uuid;

/// A database extension for the certificate utility-related operations.
pub struct CertificatesDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
}

impl<'pool> CertificatesDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Retrieves private key for the specified user with the specified ID.
    pub async fn get_private_key(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<PrivateKey>> {
        query_as!(
            RawPrivateKey,
            r#"
SELECT id, name, alg, pkcs8, encrypted, created_at, updated_at
FROM user_data_certificates_private_keys
WHERE user_id = $1 AND id = $2
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
INSERT INTO user_data_certificates_private_keys (user_id, id, name, alg, pkcs8, encrypted, created_at, updated_at)
VALUES ( $1, $2, $3, $4, $5, $6, $7, $8 )
        "#,
            *user_id,
            raw_private_key.id,
            raw_private_key.name,
            raw_private_key.alg,
            raw_private_key.pkcs8,
            raw_private_key.encrypted,
            raw_private_key.created_at,
            raw_private_key.updated_at
        )
            .execute(self.pool)
            .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::conflict(format!(
                    "Private key ('{}') already exists.",
                    private_key.name
                ))
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
SET name = $3, pkcs8 = $4, encrypted = $5, updated_at = $6
WHERE user_id = $1 AND id = $2
        "#,
            *user_id,
            raw_private_key.id,
            raw_private_key.name,
            raw_private_key.pkcs8,
            raw_private_key.encrypted,
            raw_private_key.updated_at
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
                    SecutilsError::conflict(format!(
                        "Private key ('{}') already exists.",
                        private_key.name
                    ))
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

    /// Removes private key for the specified user with the specified ID.
    pub async fn remove_private_key(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_data_certificates_private_keys
WHERE user_id = $1 AND id = $2
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
SELECT id, name, alg, ''::bytea as "pkcs8!", encrypted, created_at, updated_at
FROM user_data_certificates_private_keys
WHERE user_id = $1
ORDER BY updated_at
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

    /// Retrieves all private keys for the specified user with full pkcs8 data (for export).
    pub async fn get_private_keys_for_export(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<PrivateKey>> {
        let raw_private_keys = query_as!(
            RawPrivateKey,
            r#"
SELECT id, name, alg, pkcs8, encrypted, created_at, updated_at
FROM user_data_certificates_private_keys
WHERE user_id = $1
ORDER BY updated_at
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

    /// Retrieves private keys for the specified user matching the given IDs, with full pkcs8 data.
    pub async fn bulk_get_private_keys_for_export(
        &self,
        user_id: UserId,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<PrivateKey>> {
        let raw_private_keys = query_as!(
            RawPrivateKey,
            r#"
SELECT id, name, alg, pkcs8, encrypted, created_at, updated_at
FROM user_data_certificates_private_keys
WHERE user_id = $1 AND id = ANY($2)
ORDER BY updated_at
                "#,
            *user_id,
            ids
        )
        .fetch_all(self.pool)
        .await?;

        let mut private_keys = vec![];
        for raw_private_key in raw_private_keys {
            private_keys.push(PrivateKey::try_from(raw_private_key)?);
        }

        Ok(private_keys)
    }

    /// Retrieves certificate templates for the specified user matching the given IDs.
    pub async fn bulk_get_certificate_templates(
        &self,
        user_id: UserId,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<CertificateTemplate>> {
        let raw_templates = query_as!(
            RawCertificateTemplate,
            r#"
SELECT id, name, attributes, created_at, updated_at
FROM user_data_certificates_certificate_templates
WHERE user_id = $1 AND id = ANY($2)
ORDER BY updated_at
                "#,
            *user_id,
            ids
        )
        .fetch_all(self.pool)
        .await?;

        let mut templates = vec![];
        for raw_template in raw_templates {
            templates.push(CertificateTemplate::try_from(raw_template)?);
        }

        Ok(templates)
    }

    /// Retrieves certificate template for the specified user with the specified ID.
    pub async fn get_certificate_template(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<CertificateTemplate>> {
        query_as!(
            RawCertificateTemplate,
            r#"
SELECT id, name, attributes, created_at, updated_at
FROM user_data_certificates_certificate_templates
WHERE user_id = $1 AND id = $2
                "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(CertificateTemplate::try_from)
        .transpose()
    }

    /// Inserts certificate template.
    pub async fn insert_certificate_template(
        &self,
        user_id: UserId,
        certificate_template: &CertificateTemplate,
    ) -> anyhow::Result<()> {
        let raw_certificate_template = RawCertificateTemplate::try_from(certificate_template)?;
        let result = query!(
            r#"
INSERT INTO user_data_certificates_certificate_templates (user_id, id, name, attributes, created_at, updated_at)
VALUES ( $1, $2, $3, $4, $5, $6 )
        "#,
            *user_id,
            raw_certificate_template.id,
            raw_certificate_template.name,
            raw_certificate_template.attributes,
            raw_certificate_template.created_at,
            raw_certificate_template.updated_at
        )
            .execute(self.pool)
            .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::conflict(format!(
                    "Certificate template ('{}') already exists.",
                    certificate_template.name
                ))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create certificate template ('{}') due to unknown reason.",
                    certificate_template.name
                )))
            });
        }

        Ok(())
    }

    /// Updates certificate template.
    pub async fn update_certificate_template(
        &self,
        user_id: UserId,
        certificate_template: &CertificateTemplate,
    ) -> anyhow::Result<()> {
        let raw_certificate_template = RawCertificateTemplate::try_from(certificate_template)?;
        let result = query!(
            r#"
UPDATE user_data_certificates_certificate_templates
SET name = $3, attributes = $4, updated_at = $5
WHERE user_id = $1 AND id = $2
        "#,
            *user_id,
            raw_certificate_template.id,
            raw_certificate_template.name,
            raw_certificate_template.attributes,
            raw_certificate_template.updated_at
        )
        .execute(self.pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "A certificate template ('{}') doesn't exist.",
                        certificate_template.name
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
                        "Certificate template ('{}') already exists.",
                        certificate_template.name
                    ))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update certificate template ('{}') due to unknown reason.",
                        certificate_template.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes certificate template for the specified user with the specified ID.
    pub async fn remove_certificate_template(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_data_certificates_certificate_templates
WHERE user_id = $1 AND id = $2
                "#,
            *user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all certificate templates for the specified user.
    pub async fn get_certificate_templates(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<CertificateTemplate>> {
        let raw_certificate_templates = query_as!(
            RawCertificateTemplate,
            r#"
SELECT id, name, attributes, created_at, updated_at
FROM user_data_certificates_certificate_templates
WHERE user_id = $1
ORDER BY updated_at
                "#,
            *user_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut certificate_templates = vec![];
        for raw_certificate_template in raw_certificate_templates {
            certificate_templates.push(CertificateTemplate::try_from(raw_certificate_template)?);
        }

        Ok(certificate_templates)
    }
}

impl Database {
    /// Returns a database extension for the certificate utility-related operations.
    pub fn certificates(&self) -> CertificatesDatabaseExt<'_> {
        CertificatesDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error as SecutilsError,
        tests::{mock_user, mock_user_with_id},
        utils::certificates::{
            CertificateAttributes, CertificateTemplate, ExtendedKeyUsage, KeyUsage, PrivateKey,
            PrivateKeyAlgorithm, PrivateKeySize, SignatureAlgorithm, Version,
        },
    };
    use actix_web::ResponseError;
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    fn get_mock_certificate_attributes() -> anyhow::Result<CertificateAttributes> {
        Ok(CertificateAttributes {
            common_name: Some("cn".to_string()),
            country: Some("c".to_string()),
            state_or_province: Some("s".to_string()),
            locality: None,
            organization: None,
            organizational_unit: None,
            key_algorithm: PrivateKeyAlgorithm::Ed25519,
            signature_algorithm: SignatureAlgorithm::Md5,
            not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
            not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
            version: Version::One,
            is_ca: true,
            key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
            extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
        })
    }

    #[sqlx::test]
    async fn can_add_and_retrieve_private_keys(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
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

        assert!(
            db.certificates()
                .get_private_key(user.id, uuid!("00000000-0000-0000-0000-000000000003"))
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_private_keys_on_insert(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };

        db.certificates()
            .insert_private_key(user.id, &private_key)
            .await?;

        let insert_error = db
            .certificates()
            .insert_private_key(user.id, &private_key)
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(insert_error.status_code(), 409);
        assert_debug_snapshot!(
            insert_error.root_cause.to_string(),
            @r###""Private key ('pk-name') already exists.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_private_key_content(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                    updated_at: OffsetDateTime::from_unix_timestamp(946720820)?,
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720820)?,
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_private_keys_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(update_error.status_code(), 409);
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""Private key ('pk-name-a') already exists.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_non_existent_private_keys_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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

    #[sqlx::test]
    async fn can_remove_private_keys(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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

    #[sqlx::test]
    async fn can_retrieve_all_private_keys(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
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

    #[sqlx::test]
    async fn can_add_and_retrieve_certificate_templates(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut certificate_templates = vec![
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "ct-name-2".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for certificate_template in certificate_templates.iter() {
            db.certificates()
                .insert_certificate_template(user.id, certificate_template)
                .await?;
        }

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, certificate_templates[0].id)
            .await?
            .unwrap();
        assert_eq!(certificate_template, certificate_templates.remove(0));

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, certificate_templates[0].id)
            .await?
            .unwrap();
        assert_eq!(certificate_template, certificate_templates.remove(0));

        assert!(
            db.certificates()
                .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000003"))
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_certificate_templates_on_insert(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let certificate_template = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "ct-name".to_string(),
            attributes: get_mock_certificate_attributes()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };

        db.certificates()
            .insert_certificate_template(user.id, &certificate_template)
            .await?;

        let insert_error = db
            .certificates()
            .insert_certificate_template(user.id, &certificate_template)
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(insert_error.status_code(), 409);
        assert_debug_snapshot!(
            insert_error.root_cause.to_string(),
            @r###""Certificate template ('ct-name') already exists.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_certificate_template_content(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        db.certificates()
            .insert_certificate_template(
                user.id,
                &CertificateTemplate {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "ct-name".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await?;

        db.certificates()
            .update_certificate_template(
                user.id,
                &CertificateTemplate {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "ct-name-new".to_string(),
                    attributes: CertificateAttributes {
                        common_name: Some("cn-new".to_string()),
                        country: Some("c".to_string()),
                        state_or_province: Some("s".to_string()),
                        locality: None,
                        organization: None,
                        organizational_unit: None,
                        key_algorithm: PrivateKeyAlgorithm::Ed25519,
                        signature_algorithm: SignatureAlgorithm::Md5,
                        not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                        not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                        version: Version::One,
                        is_ca: true,
                        key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                        extended_key_usage: Some(
                            [ExtendedKeyUsage::EmailProtection].into_iter().collect(),
                        ),
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(956720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720820)?,
                },
            )
            .await?;

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            certificate_template,
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name-new".to_string(),
                attributes: CertificateAttributes {
                    common_name: Some("cn-new".to_string()),
                    country: Some("c".to_string()),
                    state_or_province: Some("s".to_string()),
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Md5,
                    not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                    not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                    version: Version::One,
                    is_ca: true,
                    key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                    extended_key_usage: Some(
                        [ExtendedKeyUsage::EmailProtection].into_iter().collect()
                    ),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720820)?,
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_certificate_templates_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let certificate_template_a = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "ct-name-a".to_string(),
            attributes: get_mock_certificate_attributes()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_certificate_template(user.id, &certificate_template_a)
            .await?;

        let certificate_template_b = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "ct-name-b".to_string(),
            attributes: get_mock_certificate_attributes()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_certificate_template(user.id, &certificate_template_b)
            .await?;

        let update_error = db
            .certificates()
            .update_certificate_template(
                user.id,
                &CertificateTemplate {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "ct-name-a".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(update_error.status_code(), 409);
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""Certificate template ('ct-name-a') already exists.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_non_existent_certificate_templates_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let update_error = db
            .certificates()
            .update_certificate_template(
                user.id,
                &CertificateTemplate {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    name: "ct-name-a".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(update_error.status_code(), 400);
        assert_debug_snapshot!(
            update_error,
            @r###""A certificate template ('ct-name-a') doesn't exist.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_certificate_templates(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut certificate_templates = vec![
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "ct-name-2".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for certificate_template in certificate_templates.iter() {
            db.certificates()
                .insert_certificate_template(user.id, certificate_template)
                .await?;
        }

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(certificate_template, certificate_templates.remove(0));

        let certificate_template_2 = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(certificate_template_2, certificate_templates[0].clone());

        db.certificates()
            .remove_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(certificate_template.is_none());

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(certificate_template, certificate_templates.remove(0));

        db.certificates()
            .remove_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(certificate_template.is_none());

        let certificate_template = db
            .certificates()
            .get_certificate_template(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(certificate_template.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_all_certificate_templates(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let certificate_templates = vec![
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "ct-name-2".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for certificate_template in certificate_templates.iter() {
            db.certificates()
                .insert_certificate_template(user.id, certificate_template)
                .await?;
        }

        assert_eq!(
            db.certificates().get_certificate_templates(user.id).await?,
            certificate_templates
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_certificate_templates_empty(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let templates = db
            .certificates()
            .bulk_get_certificate_templates(user.id, &[])
            .await?;
        assert!(templates.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_certificate_templates_returns_matching(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let templates = vec![
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                name: "ct-name-2".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946820800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000003"),
                name: "ct-name-3".to_string(),
                attributes: get_mock_certificate_attributes()?,
                created_at: OffsetDateTime::from_unix_timestamp(946920800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946920810)?,
            },
        ];

        for template in templates.iter() {
            db.certificates()
                .insert_certificate_template(user.id, template)
                .await?;
        }

        let result = db
            .certificates()
            .bulk_get_certificate_templates(
                user.id,
                &[
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    uuid!("00000000-0000-0000-0000-000000000003"),
                ],
            )
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], templates[0]);
        assert_eq!(result[1], templates[2]);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_certificate_templates_ignores_non_existent(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let template = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "ct-name".to_string(),
            attributes: get_mock_certificate_attributes()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_certificate_template(user.id, &template)
            .await?;

        let result = db
            .certificates()
            .bulk_get_certificate_templates(
                user.id,
                &[
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    uuid!("00000000-0000-0000-0000-000000000099"),
                ],
            )
            .await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], template);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_certificate_templates_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let template = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "ct-name".to_string(),
            attributes: get_mock_certificate_attributes()?,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_certificate_template(user_a.id, &template)
            .await?;

        let result = db
            .certificates()
            .bulk_get_certificate_templates(
                user_b.id,
                &[uuid!("00000000-0000-0000-0000-000000000001")],
            )
            .await?;
        assert!(result.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_private_keys_for_export_empty(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let keys = db
            .certificates()
            .get_private_keys_for_export(user.id)
            .await?;
        assert!(keys.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_private_keys_for_export_includes_pkcs8(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
        ];

        for private_key in private_keys.iter() {
            db.certificates()
                .insert_private_key(user.id, private_key)
                .await?;
        }

        let result = db
            .certificates()
            .get_private_keys_for_export(user.id)
            .await?;
        assert_eq!(result.len(), 2);
        // Unlike get_private_keys, get_private_keys_for_export includes pkcs8 data.
        assert_eq!(result[0].pkcs8, vec![1, 2, 3]);
        assert_eq!(result[1].pkcs8, vec![4, 5, 6]);
        assert_eq!(result, private_keys);

        Ok(())
    }

    #[sqlx::test]
    async fn get_private_keys_for_export_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        db.certificates()
            .insert_private_key(
                user_a.id,
                &PrivateKey {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "pk-name".to_string(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size2048,
                    },
                    pkcs8: vec![1, 2, 3],
                    encrypted: true,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await?;

        let result = db
            .certificates()
            .get_private_keys_for_export(user_b.id)
            .await?;
        assert!(result.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_private_keys_for_export_empty(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let keys = db
            .certificates()
            .bulk_get_private_keys_for_export(user.id, &[])
            .await?;
        assert!(keys.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_private_keys_for_export_returns_matching(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
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
                updated_at: OffsetDateTime::from_unix_timestamp(946820810)?,
            },
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000003"),
                name: "pk-name-3".to_string(),
                alg: PrivateKeyAlgorithm::Ed25519,
                pkcs8: vec![7, 8, 9],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946920800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946920810)?,
            },
        ];

        for private_key in private_keys.iter() {
            db.certificates()
                .insert_private_key(user.id, private_key)
                .await?;
        }

        let result = db
            .certificates()
            .bulk_get_private_keys_for_export(
                user.id,
                &[
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    uuid!("00000000-0000-0000-0000-000000000003"),
                ],
            )
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], private_keys[0]);
        assert_eq!(result[1], private_keys[2]);
        // Verify pkcs8 data is included.
        assert_eq!(result[0].pkcs8, vec![1, 2, 3]);
        assert_eq!(result[1].pkcs8, vec![7, 8, 9]);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_private_keys_for_export_ignores_non_existent(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_private_key(user.id, &private_key)
            .await?;

        let result = db
            .certificates()
            .bulk_get_private_keys_for_export(
                user.id,
                &[
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    uuid!("00000000-0000-0000-0000-000000000099"),
                ],
            )
            .await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], private_key);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_private_keys_for_export_isolated_per_user(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        db.certificates()
            .insert_private_key(
                user_a.id,
                &PrivateKey {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    name: "pk-name".to_string(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size2048,
                    },
                    pkcs8: vec![1, 2, 3],
                    encrypted: true,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            )
            .await?;

        let result = db
            .certificates()
            .bulk_get_private_keys_for_export(
                user_b.id,
                &[uuid!("00000000-0000-0000-0000-000000000001")],
            )
            .await?;
        assert!(result.is_empty());

        Ok(())
    }
}
