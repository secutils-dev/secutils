use crate::utils::certificates::PrivateKeyAlgorithm;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"keyName": "my-key", "alg": {"keyType": "ed25519"}, "tagIds": []}))]
pub struct PrivateKeysCreateParams {
    pub key_name: String,
    pub alg: PrivateKeyAlgorithm,
    pub passphrase: Option<String>,
    /// Tag IDs to assign to this private key.
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
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
                tag_ids: vec![],
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
                tag_ids: vec![],
            }
        );

        Ok(())
    }
}
