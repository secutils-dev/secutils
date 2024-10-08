use crate::utils::certificates::PrivateKey;
use time::OffsetDateTime;
use uuid::Uuid;

use super::raw_private_key_algorithm::RawPrivateKeyAlgorithm;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawPrivateKey {
    pub id: Uuid,
    pub name: String,
    pub alg: Vec<u8>,
    pub pkcs8: Vec<u8>,
    pub encrypted: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TryFrom<RawPrivateKey> for PrivateKey {
    type Error = anyhow::Error;

    fn try_from(raw: RawPrivateKey) -> Result<Self, Self::Error> {
        Ok(PrivateKey {
            id: raw.id,
            name: raw.name,
            alg: postcard::from_bytes::<RawPrivateKeyAlgorithm>(&raw.alg)?.into(),
            pkcs8: raw.pkcs8,
            encrypted: raw.encrypted,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

impl TryFrom<&PrivateKey> for RawPrivateKey {
    type Error = anyhow::Error;

    fn try_from(item: &PrivateKey) -> Result<Self, Self::Error> {
        Ok(RawPrivateKey {
            id: item.id,
            name: item.name.clone(),
            alg: postcard::to_stdvec(&RawPrivateKeyAlgorithm::from(item.alg))?,
            pkcs8: item.pkcs8.clone(),
            encrypted: item.encrypted,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawPrivateKey;
    use crate::utils::certificates::{PrivateKey, PrivateKeyAlgorithm, PrivateKeySize};
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_private_key() -> anyhow::Result<()> {
        assert_eq!(
            PrivateKey::try_from(RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        assert_eq!(
            PrivateKey::try_from(RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: false,
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_private_key() -> anyhow::Result<()> {
        assert_eq!(
            RawPrivateKey::try_from(&PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        assert_eq!(
            RawPrivateKey::try_from(&PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: false,
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:00
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }
}
