use crate::utils::certificates::PrivateKeyAlgorithm;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Describes stored private key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKey {
    /// Unique private key id (UUIDv7).
    pub id: Uuid,
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
    use crate::utils::certificates::{PrivateKey, PrivateKeyAlgorithm, PrivateKeySize};
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(
            PrivateKey {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                alg: PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size2048 },
                pkcs8: vec![1, 2, 3],
                encrypted: true,
                 // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            },
            @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
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
          "id": "00000000-0000-0000-0000-000000000001",
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
                id: uuid!("00000000-0000-0000-0000-000000000001"),
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
