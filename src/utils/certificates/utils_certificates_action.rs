use crate::utils::CertificateFormat;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesAction {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        template_name: String,
        format: CertificateFormat,
        passphrase: Option<String>,
    },
    GenerateRsaKeyPair,
}

#[cfg(test)]
mod tests {
    use crate::utils::{CertificateFormat, UtilsCertificatesAction};

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r###"
{
    "type": "generateSelfSignedCertificate",
    "value": { "templateName": "template", "format": "pem" }
}
          "###
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "template".to_string(),
                format: CertificateFormat::Pem,
                passphrase: None,
            }
        );
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r###"
{
    "type": "generateSelfSignedCertificate",
    "value": { "templateName": "template", "format": "pkcs12", "passphrase": "phrase" }
}
          "###
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "template".to_string(),
                format: CertificateFormat::Pkcs12,
                passphrase: Some("phrase".to_string()),
            }
        );
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r###"
{
    "type": "generateRsaKeyPair"
}
          "###
            )?,
            UtilsCertificatesAction::GenerateRsaKeyPair
        );

        Ok(())
    }
}
