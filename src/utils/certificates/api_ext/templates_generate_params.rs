use crate::utils::certificates::ExportFormat;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesGenerateParams {
    pub format: ExportFormat,
    pub passphrase: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::{api_ext::TemplatesGenerateParams, ExportFormat};

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<TemplatesGenerateParams>(
                r#"
        {
            "format": "pem"
        }
                  "#
            )?,
            TemplatesGenerateParams {
                format: ExportFormat::Pem,
                passphrase: None,
            }
        );
        assert_eq!(
            serde_json::from_str::<TemplatesGenerateParams>(
                r#"
        {
            "format": "pkcs12",
            "passphrase": "phrase"
        }
                  "#
            )?,
            TemplatesGenerateParams {
                format: ExportFormat::Pkcs12,
                passphrase: Some("phrase".to_string()),
            }
        );

        Ok(())
    }
}
