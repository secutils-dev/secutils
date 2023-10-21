use crate::utils::{PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize};
use serde::{Deserialize, Serialize};

/// Main `KeyAlgorithm` enum has Serde attributes that are needed for JSON serialization, but aren't
/// compatible with the `postcard`. This struct "copy" is used only for the `postcard` serialization
/// and deserialization.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub(super) enum RawPrivateKeyAlgorithm {
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

#[cfg(test)]
mod tests {
    use super::RawPrivateKeyAlgorithm;
    use crate::utils::{PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize};

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
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&RawPrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            })?,
            vec![0, 1]
        );

        assert_eq!(
            postcard::to_stdvec(&RawPrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            })?,
            vec![1, 1]
        );

        assert_eq!(
            postcard::to_stdvec(&RawPrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            })?,
            vec![2, 0]
        );

        assert_eq!(
            postcard::to_stdvec(&RawPrivateKeyAlgorithm::Ed25519)?,
            vec![3]
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<RawPrivateKeyAlgorithm>(&[0, 1])?,
            RawPrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            }
        );

        assert_eq!(
            postcard::from_bytes::<RawPrivateKeyAlgorithm>(&[1, 1])?,
            RawPrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            }
        );

        assert_eq!(
            postcard::from_bytes::<RawPrivateKeyAlgorithm>(&[2, 0])?,
            RawPrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            }
        );

        assert_eq!(
            postcard::from_bytes::<RawPrivateKeyAlgorithm>(&[3])?,
            RawPrivateKeyAlgorithm::Ed25519
        );

        Ok(())
    }
}
