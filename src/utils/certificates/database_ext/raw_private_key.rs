use crate::utils::certificates::PrivateKey;
use time::OffsetDateTime;
use uuid::Uuid;

use super::raw_private_key_algorithm::RawPrivateKeyAlgorithm;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawPrivateKey {
    pub id: Vec<u8>,
    pub name: String,
    pub alg: Vec<u8>,
    pub pkcs8: Vec<u8>,
    pub encrypted: i64,
    pub created_at: i64,
}

impl TryFrom<RawPrivateKey> for PrivateKey {
    type Error = anyhow::Error;

    fn try_from(raw: RawPrivateKey) -> Result<Self, Self::Error> {
        Ok(PrivateKey {
            id: Uuid::from_slice(raw.id.as_slice())?,
            name: raw.name,
            alg: postcard::from_bytes::<RawPrivateKeyAlgorithm>(&raw.alg)?.into(),
            pkcs8: raw.pkcs8,
            encrypted: raw.encrypted > 0,
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl TryFrom<&PrivateKey> for RawPrivateKey {
    type Error = anyhow::Error;

    fn try_from(item: &PrivateKey) -> Result<Self, Self::Error> {
        Ok(RawPrivateKey {
            id: item.id.into(),
            name: item.name.clone(),
            alg: postcard::to_stdvec(&RawPrivateKeyAlgorithm::from(item.alg))?,
            pkcs8: item.pkcs8.clone(),
            encrypted: item.encrypted as i64,
            created_at: item.created_at.unix_timestamp(),
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
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: 1,
                // January 1, 2000 10:00:00
                created_at: 946720800,
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
            }
        );

        assert_eq!(
            PrivateKey::try_from(RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: 0,
                // January 1, 2000 10:00:00
                created_at: 946720800,
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
            })?,
            RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: 1,
                // January 1, 2000 10:00:00
                created_at: 946720800,
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
            })?,
            RawPrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: 0,
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }
}
