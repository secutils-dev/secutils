use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKeysUpdateParams {
    pub key_name: Option<String>,
    pub new_passphrase: Option<String>,
    pub passphrase: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::PrivateKeysUpdateParams;

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
            }
        );

        Ok(())
    }
}
