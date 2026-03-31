use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"keyName": "renamed-key"}))]
pub struct PrivateKeysUpdateParams {
    pub key_name: Option<String>,
    pub new_passphrase: Option<String>,
    pub passphrase: Option<String>,
    /// Tag IDs to assign. When `Some`, replaces all tags; when `None`, tags are unchanged.
    pub tag_ids: Option<Vec<Uuid>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::api_ext::PrivateKeysUpdateParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PrivateKeysUpdateParams>(
                r#"
{
    "passphrase": "phrase", 
    "newPassphrase": "phrase_new"
}
          "#
            )?,
            PrivateKeysUpdateParams {
                key_name: None,
                passphrase: Some("phrase".to_string()),
                new_passphrase: Some("phrase_new".to_string()),
                tag_ids: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeysUpdateParams>(
                r#"
{
    "keyName": "pk",
    "passphrase": "phrase",
    "newPassphrase": "phrase_new"
}
          "#
            )?,
            PrivateKeysUpdateParams {
                key_name: Some("pk".to_string()),
                passphrase: Some("phrase".to_string()),
                new_passphrase: Some("phrase_new".to_string()),
                tag_ids: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeysUpdateParams>(
                r#"
{
    "keyName": "pk"
}
          "#
            )?,
            PrivateKeysUpdateParams {
                key_name: Some("pk".to_string()),
                passphrase: None,
                new_passphrase: None,
                tag_ids: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeysUpdateParams>(
                r#"
{
}
          "#
            )?,
            PrivateKeysUpdateParams {
                key_name: None,
                passphrase: None,
                new_passphrase: None,
                tag_ids: None,
            }
        );

        Ok(())
    }
}
