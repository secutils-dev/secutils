use crate::utils::CertificateAttributes;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesUpdateParams {
    pub template_name: Option<String>,
    pub attributes: Option<CertificateAttributes>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        CertificateAttributes, ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm, SignatureAlgorithm,
        TemplatesUpdateParams, Version,
    };
    use time::OffsetDateTime;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<TemplatesUpdateParams>(
                r#"
        {
            "templateName": "ct",
            "attributes": {
                "commonName": "CA Issuer",
                "keyAlgorithm": {
                    "keyType": "ed25519"
                },
                "signatureAlgorithm": "ed25519",
                "notValidBefore": 946720800,
                "notValidAfter": 1262340000,
                "version": 3,
                "isCa": true,
                "keyUsage": ["crlSigning"],
                "extendedKeyUsage": ["tlsWebServerAuthentication"]
            }
        }
                  "#
            )?,
            TemplatesUpdateParams {
                template_name: Some("ct".to_string()),
                attributes: Some(CertificateAttributes {
                    common_name: Some("CA Issuer".to_string()),
                    country: None,
                    state_or_province: None,
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Ed25519,
                    not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                    not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                    version: Version::Three,
                    is_ca: true,
                    key_usage: Some([KeyUsage::CrlSigning].into_iter().collect()),
                    extended_key_usage: Some(
                        [ExtendedKeyUsage::TlsWebServerAuthentication]
                            .into_iter()
                            .collect()
                    ),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<TemplatesUpdateParams>(
                r#"
        {
            "attributes": {
                "commonName": "CA Issuer",
                "keyAlgorithm": {
                  "keyType": "ed25519"
                },
                "signatureAlgorithm": "ed25519",
                "notValidBefore": 946720800,
                "notValidAfter": 1262340000,
                "version": 3,
                "isCa": true,
                "keyUsage": ["crlSigning"],
                "extendedKeyUsage": ["tlsWebServerAuthentication"]
            }
        }
                  "#
            )?,
            TemplatesUpdateParams {
                template_name: None,
                attributes: Some(CertificateAttributes {
                    common_name: Some("CA Issuer".to_string()),
                    country: None,
                    state_or_province: None,
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Ed25519,
                    not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                    not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                    version: Version::Three,
                    is_ca: true,
                    key_usage: Some([KeyUsage::CrlSigning].into_iter().collect()),
                    extended_key_usage: Some(
                        [ExtendedKeyUsage::TlsWebServerAuthentication]
                            .into_iter()
                            .collect()
                    ),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<TemplatesUpdateParams>(
                r#"
        {
            "templateName": "ct"
        }
                  "#
            )?,
            TemplatesUpdateParams {
                template_name: Some("ct".to_string()),
                attributes: None
            }
        );

        Ok(())
    }
}
