use crate::utils::certificates::ExportFormat;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"format": "pem"}))]
pub struct TemplatesGenerateParams {
    pub format: ExportFormat,
    pub passphrase: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::{ExportFormat, api_ext::TemplatesGenerateParams};

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
