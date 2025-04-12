use crate::utils::certificates::PrivateKeyAlgorithm;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKeysCreateParams {
    pub key_name: String,
    pub alg: PrivateKeyAlgorithm,
    pub passphrase: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::{
        PrivateKeyAlgorithm, PrivateKeySize, api_ext::PrivateKeysCreateParams,
    };

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PrivateKeysCreateParams>(
                r#"
{
    "keyName": "pk",
    "alg": {"keyType": "rsa", "keySize": "1024"},
    "passphrase": "phrase"
}
          "#
            )?,
            PrivateKeysCreateParams {
                key_name: "pk".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024
                },
                passphrase: Some("phrase".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeysCreateParams>(
                r#"
{
    "keyName": "pk",
    "alg": {"keyType": "rsa", "keySize": "1024"}
}
          "#
            )?,
            PrivateKeysCreateParams {
                key_name: "pk".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024
                },
                passphrase: None,
            }
        );

        Ok(())
    }
}
