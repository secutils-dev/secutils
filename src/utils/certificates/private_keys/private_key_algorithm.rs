use crate::utils::certificates::{PrivateKeyEllipticCurve, PrivateKeySize};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "keyType")]
pub enum PrivateKeyAlgorithm {
    #[serde(rename_all = "camelCase")]
    Rsa {
        key_size: PrivateKeySize,
    },
    #[serde(rename_all = "camelCase")]
    Dsa {
        key_size: PrivateKeySize,
    },
    Ecdsa {
        curve: PrivateKeyEllipticCurve,
    },
    Ed25519,
}

impl Display for PrivateKeyAlgorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PrivateKeyAlgorithm::Rsa { key_size } => write!(f, "RSA-{key_size}"),
            PrivateKeyAlgorithm::Dsa { key_size } => write!(f, "DSA-{key_size}"),
            PrivateKeyAlgorithm::Ecdsa { curve } => write!(f, "ECDSA-{curve}"),
            PrivateKeyAlgorithm::Ed25519 => write!(f, "ED25519"),
        }
    }
}

impl FromStr for PrivateKeyAlgorithm {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uppercase_s = s.to_uppercase();
        let parts: Vec<&str> = uppercase_s.split('-').collect();
        match parts.as_slice() {
            ["RSA", key_size] => Ok(PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::from_str(key_size)?,
            }),
            ["DSA", key_size] => Ok(PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::from_str(key_size)?,
            }),
            ["ECDSA", curve] => Ok(PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::from_str(curve)?,
            }),
            ["ED25519"] => Ok(PrivateKeyAlgorithm::Ed25519),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::{
        PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize,
    };
    use insta::{assert_json_snapshot, assert_snapshot};
    use std::str::FromStr;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size1024 }, @r###"
        {
          "keyType": "rsa",
          "keySize": "1024"
        }
        "###);
        assert_json_snapshot!(PrivateKeyAlgorithm::Dsa { key_size: PrivateKeySize::Size2048 }, @r###"
        {
          "keyType": "dsa",
          "keySize": "2048"
        }
        "###);
        assert_json_snapshot!(PrivateKeyAlgorithm::Ecdsa { curve: PrivateKeyEllipticCurve::SECP256R1 }, @r###"
        {
          "keyType": "ecdsa",
          "curve": "secp256r1"
        }
        "###);
        assert_json_snapshot!(PrivateKeyAlgorithm::Ed25519, @r###"
        {
          "keyType": "ed25519"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PrivateKeyAlgorithm>(
                r#"{ "keyType": "rsa", "keySize": "1024" }"#
            )?,
            PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024
            }
        );
        assert_eq!(
            serde_json::from_str::<PrivateKeyAlgorithm>(
                r#"{ "keyType": "dsa", "keySize": "2048" }"#
            )?,
            PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            }
        );
        assert_eq!(
            serde_json::from_str::<PrivateKeyAlgorithm>(
                r#"{ "keyType": "ecdsa", "curve": "secp256r1" }"#
            )?,
            PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            }
        );
        assert_eq!(
            serde_json::from_str::<PrivateKeyAlgorithm>(r#"{ "keyType": "ed25519" }"#)?,
            PrivateKeyAlgorithm::Ed25519
        );

        Ok(())
    }

    #[test]
    fn string_representation() -> anyhow::Result<()> {
        assert_snapshot!(PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size1024 }, @"RSA-1024");
        assert_snapshot!(PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size2048 }, @"RSA-2048");
        assert_snapshot!(PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size4096 }, @"RSA-4096");
        assert_snapshot!(PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size8192 }, @"RSA-8192");
        assert_snapshot!(PrivateKeyAlgorithm::Dsa { key_size: PrivateKeySize::Size1024 }, @"DSA-1024");
        assert_snapshot!(PrivateKeyAlgorithm::Dsa { key_size: PrivateKeySize::Size2048 }, @"DSA-2048");
        assert_snapshot!(PrivateKeyAlgorithm::Dsa { key_size: PrivateKeySize::Size4096 }, @"DSA-4096");
        assert_snapshot!(PrivateKeyAlgorithm::Dsa { key_size: PrivateKeySize::Size8192 }, @"DSA-8192");
        assert_snapshot!(PrivateKeyAlgorithm::Ecdsa { curve: PrivateKeyEllipticCurve::SECP256R1 }, @"ECDSA-SECP256R1");
        assert_snapshot!(PrivateKeyAlgorithm::Ecdsa { curve: PrivateKeyEllipticCurve::SECP384R1 }, @"ECDSA-SECP384R1");
        assert_snapshot!(PrivateKeyAlgorithm::Ecdsa { curve: PrivateKeyEllipticCurve::SECP521R1 }, @"ECDSA-SECP521R1");
        assert_snapshot!(PrivateKeyAlgorithm::Ed25519, @"ED25519");

        assert_eq!(
            PrivateKeyAlgorithm::from_str("RSA-1024"),
            Ok(PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("RSA-2048"),
            Ok(PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("RSA-4096"),
            Ok(PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size4096
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("RSA-8192"),
            Ok(PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size8192
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("DSA-1024"),
            Ok(PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size1024
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("DSA-2048"),
            Ok(PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("DSA-4096"),
            Ok(PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size4096
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("DSA-8192"),
            Ok(PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size8192
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("ECDSA-SECP256R1"),
            Ok(PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("ECDSA-SECP384R1"),
            Ok(PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP384R1
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("ECDSA-SECP521R1"),
            Ok(PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP521R1
            })
        );
        assert_eq!(
            PrivateKeyAlgorithm::from_str("Ed25519"),
            Ok(PrivateKeyAlgorithm::Ed25519)
        );

        Ok(())
    }
}
