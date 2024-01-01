use crate::utils::certificates::{PrivateKeyEllipticCurve, PrivateKeySize};
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use crate::utils::certificates::{
        PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize,
    };
    use insta::assert_json_snapshot;

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
}
