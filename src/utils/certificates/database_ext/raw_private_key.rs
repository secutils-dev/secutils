use crate::utils::{PrivateKey, PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Main `KeyAlgorithm` enum has Serde attributes that are needed fro JSON serialization, but aren't
/// compatible with the `postcard`.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
enum RawPrivateKeyAlgorithm {
    Rsa { key_size: PrivateKeySize },
    Dsa { key_size: PrivateKeySize },
    Ecdsa { curve: PrivateKeyEllipticCurve },
    Ed25519,
}

impl From<RawPrivateKeyAlgorithm> for PrivateKeyAlgorithm {
    fn from(raw: RawPrivateKeyAlgorithm) -> Self {
        match raw {
            RawPrivateKeyAlgorithm::Rsa { key_size } => PrivateKeyAlgorithm::Rsa { key_size },
            RawPrivateKeyAlgorithm::Dsa { key_size } => PrivateKeyAlgorithm::Dsa { key_size },
            RawPrivateKeyAlgorithm::Ecdsa { curve } => PrivateKeyAlgorithm::Ecdsa { curve },
            RawPrivateKeyAlgorithm::Ed25519 => PrivateKeyAlgorithm::Ed25519,
        }
    }
}

impl From<PrivateKeyAlgorithm> for RawPrivateKeyAlgorithm {
    fn from(item: PrivateKeyAlgorithm) -> Self {
        match item {
            PrivateKeyAlgorithm::Rsa { key_size } => RawPrivateKeyAlgorithm::Rsa { key_size },
            PrivateKeyAlgorithm::Dsa { key_size } => RawPrivateKeyAlgorithm::Dsa { key_size },
            PrivateKeyAlgorithm::Ecdsa { curve } => RawPrivateKeyAlgorithm::Ecdsa { curve },
            PrivateKeyAlgorithm::Ed25519 => RawPrivateKeyAlgorithm::Ed25519,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawPrivateKey {
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
    use super::{RawPrivateKey, RawPrivateKeyAlgorithm};
    use crate::utils::{PrivateKey, PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize};
    use time::OffsetDateTime;

    #[test]
    fn can_convert_to_key_algorithm() -> anyhow::Result<()> {
        assert_eq!(
            PrivateKeyAlgorithm::from(RawPrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            }),
            PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            }
        );

        assert_eq!(
            PrivateKeyAlgorithm::from(RawPrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            }),
            PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            }
        );

        assert_eq!(
            PrivateKeyAlgorithm::from(RawPrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            }),
            PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            }
        );

        assert_eq!(
            PrivateKeyAlgorithm::from(RawPrivateKeyAlgorithm::Ed25519),
            PrivateKeyAlgorithm::Ed25519
        );

        Ok(())
    }

    #[test]
    fn can_convert_to_raw_key_algorithm() -> anyhow::Result<()> {
        assert_eq!(
            RawPrivateKeyAlgorithm::from(PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            }),
            RawPrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            }
        );

        assert_eq!(
            RawPrivateKeyAlgorithm::from(PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            }),
            RawPrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            }
        );

        assert_eq!(
            RawPrivateKeyAlgorithm::from(PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            }),
            RawPrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            }
        );

        assert_eq!(
            RawPrivateKeyAlgorithm::from(PrivateKeyAlgorithm::Ed25519),
            RawPrivateKeyAlgorithm::Ed25519
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_private_key() -> anyhow::Result<()> {
        assert_eq!(
            PrivateKey::try_from(RawPrivateKey {
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: 1,
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            PrivateKey {
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
                name: "pk-name".to_string(),
                alg: vec![0, 1],
                pkcs8: vec![1, 2, 3],
                encrypted: 0,
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            PrivateKey {
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
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawPrivateKey {
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
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: false,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawPrivateKey {
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
