use crate::utils::{utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, CertificateFormat};
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

impl UtilsCertificatesAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsCertificatesAction::GenerateSelfSignedCertificate { template_name, .. } => {
                if template_name.is_empty() {
                    anyhow::bail!("Template name cannot be empty");
                }

                if template_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Template name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }
            }
            UtilsCertificatesAction::GenerateRsaKeyPair => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{CertificateFormat, UtilsCertificatesAction};
    use insta::assert_debug_snapshot;

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

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsCertificatesAction::GenerateRsaKeyPair
            .validate()
            .is_ok());

        assert!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "a".repeat(100),
            format: CertificateFormat::Pem,
            passphrase: None,
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "".to_string(),
            format: CertificateFormat::Pem,
            passphrase: None,
        }.validate(), @r###"
        Err(
            "Template name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "a".repeat(101),
            format: CertificateFormat::Pem,
            passphrase: None,
        }.validate(), @r###"
        Err(
            "Template name cannot be longer than 100 characters",
        )
        "###);

        Ok(())
    }
}
