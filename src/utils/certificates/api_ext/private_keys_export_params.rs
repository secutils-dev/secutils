use crate::utils::ExportFormat;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKeysExportParams {
    pub format: ExportFormat,
    pub passphrase: Option<String>,
    pub export_passphrase: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{ExportFormat, PrivateKeysExportParams};

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PrivateKeysExportParams>(
                r#"
{
    "format": "pem", 
    "passphrase": "phrase", 
    "exportPassphrase": "phrase_new"
}
          "#
            )?,
            PrivateKeysExportParams {
                format: ExportFormat::Pem,
                passphrase: Some("phrase".to_string()),
                export_passphrase: Some("phrase_new".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeysExportParams>(
                r#"
{
    "format": "pem"
}
          "#
            )?,
            PrivateKeysExportParams {
                format: ExportFormat::Pem,
                passphrase: None,
                export_passphrase: None,
            }
        );

        Ok(())
    }
}
