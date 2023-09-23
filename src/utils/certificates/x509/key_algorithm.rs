use crate::utils::{EllipticCurve, KeySize};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "alg")]
pub enum KeyAlgorithm {
    #[serde(rename_all = "camelCase")]
    Rsa {
        key_size: KeySize,
    },
    #[serde(rename_all = "camelCase")]
    Dsa {
        key_size: KeySize,
    },
    Ecdsa {
        curve: EllipticCurve,
    },
    Ed25519,
}

#[cfg(test)]
mod tests {
    use crate::utils::{EllipticCurve, KeyAlgorithm, KeySize};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(KeyAlgorithm::Rsa { key_size: KeySize::Size1024 }, @r###"
        {
          "alg": "rsa",
          "keySize": "1024"
        }
        "###);
        assert_json_snapshot!(KeyAlgorithm::Dsa { key_size: KeySize::Size2048 }, @r###"
        {
          "alg": "dsa",
          "keySize": "2048"
        }
        "###);
        assert_json_snapshot!(KeyAlgorithm::Ecdsa { curve: EllipticCurve::SECP256R1 }, @r###"
        {
          "alg": "ecdsa",
          "curve": "secp256r1"
        }
        "###);
        assert_json_snapshot!(KeyAlgorithm::Ed25519, @r###"
        {
          "alg": "ed25519"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r#"{ "alg": "rsa", "keySize": "1024" }"#)?,
            KeyAlgorithm::Rsa {
                key_size: KeySize::Size1024
            }
        );
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r#"{ "alg": "dsa", "keySize": "2048" }"#)?,
            KeyAlgorithm::Dsa {
                key_size: KeySize::Size2048
            }
        );
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r#"{ "alg": "ecdsa", "curve": "secp256r1" }"#)?,
            KeyAlgorithm::Ecdsa {
                curve: EllipticCurve::SECP256R1
            }
        );
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r#"{ "alg": "ed25519" }"#)?,
            KeyAlgorithm::Ed25519
        );

        Ok(())
    }
}
