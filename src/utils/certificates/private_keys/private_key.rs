use crate::utils::PrivateKeyAlgorithm;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Describes stored private key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKey {
    /// Arbitrary name of the private key.
    pub name: String,
    /// Algorithm of the private key (RSA, DSA, etc.).
    pub alg: PrivateKeyAlgorithm,
    /// Private key serialized to PKCS#8 format (with or without encryption).
    pub pkcs8: Vec<u8>,
    /// Indicates whether the private key is encrypted.
    pub encrypted: bool,
    /// Date and time when the private key was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::utils::{PrivateKey, PrivateKeyAlgorithm, PrivateKeySize};
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(
            PrivateKey {
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size2048 },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                 // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            },
            @r###"
        {
          "name": "pk-name",
          "alg": {
            "keyType": "rsa",
            "keySize": "2048"
          },
          "pkcs8": [
            1,
            2,
            3
          ],
          "encrypted": true,
          "createdAt": 946720800
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PrivateKey>(
                r#"
        {
          "name": "pk-name",
          "alg": {
            "keyType": "rsa",
            "keySize": "2048"
          },
          "pkcs8": [
            1,
            2,
            3
          ],
          "encrypted": true,
          "createdAt": 946720800
        }
        "#
            )?,
            PrivateKey {
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048
                },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            },
        );

        Ok(())
    }
}
