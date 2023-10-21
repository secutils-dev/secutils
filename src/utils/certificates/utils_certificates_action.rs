use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, CertificateAttributes, ExportFormat,
        PrivateKeyAlgorithm, UtilsCertificatesActionResult,
    },
};
use anyhow::bail;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesAction {
    GetPrivateKeys,
    #[serde(rename_all = "camelCase")]
    CreatePrivateKey {
        key_name: String,
        alg: PrivateKeyAlgorithm,
        passphrase: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    UpdatePrivateKey {
        key_id: Uuid,
        key_name: Option<String>,
        passphrase: Option<String>,
        new_passphrase: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    RemovePrivateKey {
        key_id: Uuid,
    },
    #[serde(rename_all = "camelCase")]
    ExportPrivateKey {
        key_id: Uuid,
        format: ExportFormat,
        passphrase: Option<String>,
        export_passphrase: Option<String>,
    },
    GetCertificateTemplates,
    #[serde(rename_all = "camelCase")]
    CreateCertificateTemplate {
        template_name: String,
        attributes: CertificateAttributes,
    },
    #[serde(rename_all = "camelCase")]
    UpdateCertificateTemplate {
        template_id: Uuid,
        template_name: Option<String>,
        attributes: Option<CertificateAttributes>,
    },
    #[serde(rename_all = "camelCase")]
    RemoveCertificateTemplate {
        template_id: Uuid,
    },
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        template_id: Uuid,
        format: ExportFormat,
        passphrase: Option<String>,
    },
}

impl UtilsCertificatesAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        let assert_private_key_name = |name: &str| -> Result<(), SecutilsError> {
            if name.is_empty() {
                return Err(SecutilsError::client("Private key name cannot be empty."));
            }

            if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                return Err(SecutilsError::client(format!(
                    "Private key name cannot be longer than {} characters.",
                    MAX_UTILS_ENTITY_NAME_LENGTH
                )));
            }

            Ok(())
        };

        let assert_certificate_template_name = |name: &str| -> Result<(), SecutilsError> {
            if name.is_empty() {
                return Err(SecutilsError::client(
                    "Certificate template name cannot be empty.",
                ));
            }

            if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                return Err(SecutilsError::client(format!(
                    "Certificate template name cannot be longer than {} characters.",
                    MAX_UTILS_ENTITY_NAME_LENGTH
                )));
            }

            Ok(())
        };

        match self {
            UtilsCertificatesAction::CreatePrivateKey { key_name, .. } => {
                assert_private_key_name(key_name)?;
            }
            UtilsCertificatesAction::UpdatePrivateKey {
                key_name,
                passphrase,
                new_passphrase,
                key_id,
            } => {
                let includes_new_passphrase = passphrase.is_some() || new_passphrase.is_some();
                if let Some(name) = key_name {
                    assert_private_key_name(name)?;
                } else if !includes_new_passphrase {
                    bail!(SecutilsError::client(format!(
                        "Either new name or passphrase should be provided ({key_id})."
                    )));
                }

                if includes_new_passphrase && passphrase == new_passphrase {
                    bail!(SecutilsError::client(format!(
                        "New private key passphrase should be different from the current passphrase ({key_id})."
                    )));
                }
            }
            UtilsCertificatesAction::CreateCertificateTemplate { template_name, .. } => {
                assert_certificate_template_name(template_name)?;
            }
            UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id,
                template_name,
                attributes,
            } => {
                if let Some(name) = template_name {
                    assert_certificate_template_name(name)?;
                } else if !attributes.is_some() {
                    bail!(SecutilsError::client(format!(
                        "Either new name or attributes should be provided ({template_id})."
                    )));
                }
            }
            UtilsCertificatesAction::GetPrivateKeys
            | UtilsCertificatesAction::RemovePrivateKey { .. }
            | UtilsCertificatesAction::ExportPrivateKey { .. }
            | UtilsCertificatesAction::GetCertificateTemplates
            | UtilsCertificatesAction::RemoveCertificateTemplate { .. }
            | UtilsCertificatesAction::GenerateSelfSignedCertificate { .. } => {}
        }

        Ok(())
    }

    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsCertificatesActionResult> {
        let certificates = api.certificates();
        match self {
            UtilsCertificatesAction::GetPrivateKeys => {
                Ok(UtilsCertificatesActionResult::GetPrivateKeys(
                    certificates.get_private_keys(user.id).await?,
                ))
            }
            UtilsCertificatesAction::CreatePrivateKey {
                key_name,
                alg,
                passphrase,
            } => Ok(UtilsCertificatesActionResult::CreatePrivateKey(
                certificates
                    .create_private_key(user.id, &key_name, alg, passphrase.as_deref())
                    .await?,
            )),
            UtilsCertificatesAction::UpdatePrivateKey {
                key_id,
                key_name,
                passphrase,
                new_passphrase,
            } => {
                certificates
                    .update_private_key(
                        user.id,
                        key_id,
                        key_name.as_deref(),
                        passphrase.as_deref(),
                        new_passphrase.as_deref(),
                    )
                    .await?;
                Ok(UtilsCertificatesActionResult::UpdatePrivateKey)
            }
            UtilsCertificatesAction::ExportPrivateKey {
                key_id,
                passphrase,
                export_passphrase,
                format,
            } => Ok(UtilsCertificatesActionResult::ExportPrivateKey(
                certificates
                    .export_private_key(
                        user.id,
                        key_id,
                        format,
                        passphrase.as_deref(),
                        export_passphrase.as_deref(),
                    )
                    .await?,
            )),
            UtilsCertificatesAction::RemovePrivateKey { key_id } => {
                certificates.remove_private_key(user.id, key_id).await?;
                Ok(UtilsCertificatesActionResult::RemovePrivateKey)
            }
            UtilsCertificatesAction::GetCertificateTemplates => {
                Ok(UtilsCertificatesActionResult::GetCertificateTemplates(
                    certificates.get_certificate_templates(user.id).await?,
                ))
            }
            UtilsCertificatesAction::CreateCertificateTemplate {
                template_name,
                attributes,
            } => Ok(UtilsCertificatesActionResult::CreateCertificateTemplate(
                certificates
                    .create_certificate_template(user.id, template_name, attributes)
                    .await?,
            )),
            UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id,
                template_name,
                attributes,
            } => {
                certificates
                    .update_certificate_template(user.id, template_id, template_name, attributes)
                    .await?;
                Ok(UtilsCertificatesActionResult::UpdateCertificateTemplate)
            }
            UtilsCertificatesAction::RemoveCertificateTemplate { template_id } => {
                certificates
                    .remove_certificate_template(user.id, template_id)
                    .await?;
                Ok(UtilsCertificatesActionResult::RemoveCertificateTemplate)
            }
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_id,
                format,
                passphrase,
            } => Ok(
                UtilsCertificatesActionResult::GenerateSelfSignedCertificate(
                    certificates
                        .generate_self_signed_certificate(
                            user.id,
                            template_id,
                            format,
                            passphrase.as_deref(),
                        )
                        .await?,
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockCertificateAttributes,
        utils::{
            CertificateAttributes, ExportFormat, ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm,
            PrivateKeySize, SignatureAlgorithm, UtilsCertificatesAction, Version,
        },
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "getPrivateKeys"
}
          "#
            )?,
            UtilsCertificatesAction::GetPrivateKeys
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "createPrivateKey",
    "value": { "keyName": "pk", "alg": {"keyType": "rsa", "keySize": "1024"}, "passphrase": "phrase" }
}
          "#
            )?,
            UtilsCertificatesAction::CreatePrivateKey {
                key_name: "pk".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024
                },
                passphrase: Some("phrase".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "createPrivateKey",
    "value": { "keyName": "pk", "alg": {"keyType": "rsa", "keySize": "1024"} }
}
          "#
            )?,
            UtilsCertificatesAction::CreatePrivateKey {
                key_name: "pk".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024
                },
                passphrase: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "updatePrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001", "passphrase": "phrase", "newPassphrase": "phrase_new" }
}
          "#
            )?,
            UtilsCertificatesAction::UpdatePrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                key_name: None,
                passphrase: Some("phrase".to_string()),
                new_passphrase: Some("phrase_new".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "updatePrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001", "keyName": "pk", "passphrase": "phrase", "newPassphrase": "phrase_new" }
}
          "#
            )?,
            UtilsCertificatesAction::UpdatePrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                key_name: Some("pk".to_string()),
                passphrase: Some("phrase".to_string()),
                new_passphrase: Some("phrase_new".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "updatePrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001", "keyName": "pk" }
}
          "#
            )?,
            UtilsCertificatesAction::UpdatePrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                key_name: Some("pk".to_string()),
                passphrase: None,
                new_passphrase: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "updatePrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001" }
}
          "#
            )?,
            UtilsCertificatesAction::UpdatePrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                key_name: None,
                passphrase: None,
                new_passphrase: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "removePrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001" }
}
          "#
            )?,
            UtilsCertificatesAction::RemovePrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001")
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "exportPrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001", "format": "pem", "passphrase": "phrase", "exportPassphrase": "phrase_new" }
}
          "#
            )?,
            UtilsCertificatesAction::ExportPrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                format: ExportFormat::Pem,
                passphrase: Some("phrase".to_string()),
                export_passphrase: Some("phrase_new".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "exportPrivateKey",
    "value": { "keyId": "00000000-0000-0000-0000-000000000001", "format": "pem" }
}
          "#
            )?,
            UtilsCertificatesAction::ExportPrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                format: ExportFormat::Pem,
                passphrase: None,
                export_passphrase: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "getCertificateTemplates"
}
          "#
            )?,
            UtilsCertificatesAction::GetCertificateTemplates
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "createCertificateTemplate",
    "value": { 
      "templateName": "ct",
      "attributes": {
        "commonName": "CA Issuer",
        "keyAlgorithm": { "keyType": "ed25519" },
        "signatureAlgorithm": "ed25519",
        "notValidBefore": 946720800,
        "notValidAfter": 1262340000,
        "version": 3,
        "isCa": true,
        "keyUsage": ["crlSigning"],
        "extendedKeyUsage": ["tlsWebServerAuthentication"]
      }
    }
}
          "#
            )?,
            UtilsCertificatesAction::CreateCertificateTemplate {
                template_name: "ct".to_string(),
                attributes: CertificateAttributes {
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
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
        {
            "type": "updateCertificateTemplate",
            "value": {
              "templateId": "00000000-0000-0000-0000-000000000001",
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
        }
                  "#
            )?,
            UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001"),
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
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
        {
            "type": "updateCertificateTemplate",
            "value": {
              "templateId": "00000000-0000-0000-0000-000000000001",
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
        }
                  "#
            )?,
            UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001"),
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
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
        {
            "type": "updateCertificateTemplate",
            "value": {
              "templateId": "00000000-0000-0000-0000-000000000001",
              "templateName": "ct"
            }
        }
                  "#
            )?,
            UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001"),
                template_name: Some("ct".to_string()),
                attributes: None
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
        {
            "type": "removeCertificateTemplate",
            "value": { "templateId": "00000000-0000-0000-0000-000000000001" }
        }
                  "#
            )?,
            UtilsCertificatesAction::RemoveCertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001")
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
        {
            "type": "generateSelfSignedCertificate",
            "value": { "templateId": "00000000-0000-0000-0000-000000000001", "format": "pem" }
        }
                  "#
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001"),
                format: ExportFormat::Pem,
                passphrase: None,
            }
        );
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
        {
            "type": "generateSelfSignedCertificate",
            "value": { "templateId": "00000000-0000-0000-0000-000000000001", "format": "pkcs12", "passphrase": "phrase" }
        }
                  "#
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001"),
                format: ExportFormat::Pkcs12,
                passphrase: Some("phrase".to_string()),
            }
        );

        Ok(())
    }

    #[test]
    fn validation() -> anyhow::Result<()> {
        let get_private_keys_actions_with_name = |key_name: String| {
            vec![
                UtilsCertificatesAction::CreatePrivateKey {
                    key_name: key_name.clone(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    },
                    passphrase: Some("phrase".to_string()),
                },
                UtilsCertificatesAction::UpdatePrivateKey {
                    key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                    key_name: Some(key_name),
                    passphrase: None,
                    new_passphrase: None,
                },
            ]
        };

        for action in get_private_keys_actions_with_name("a".repeat(100)) {
            assert!(action.validate().is_ok());
        }

        for action in get_private_keys_actions_with_name("".to_string()) {
            assert_eq!(
                action.validate().map_err(|err| err.to_string()),
                Err("Private key name cannot be empty.".to_string())
            );
        }

        for action in get_private_keys_actions_with_name("a".repeat(101)) {
            assert_eq!(
                action.validate().map_err(|err| err.to_string()),
                Err("Private key name cannot be longer than 100 characters.".to_string())
            );
        }

        let update_private_key_action = UtilsCertificatesAction::UpdatePrivateKey {
            key_id: uuid!("00000000-0000-0000-0000-000000000001"),
            key_name: Some("pk".to_string()),
            passphrase: Some("pass".to_string()),
            new_passphrase: Some("pass".to_string()),
        };
        assert_eq!(
            update_private_key_action.validate().map_err(|err| err.to_string()),
            Err("New private key passphrase should be different from the current passphrase (00000000-0000-0000-0000-000000000001).".to_string())
        );

        let update_private_key_action = UtilsCertificatesAction::UpdatePrivateKey {
            key_id: uuid!("00000000-0000-0000-0000-000000000001"),
            key_name: None,
            passphrase: None,
            new_passphrase: None,
        };
        assert_eq!(
            update_private_key_action.validate().map_err(|err| err.to_string()),
            Err("Either new name or passphrase should be provided (00000000-0000-0000-0000-000000000001).".to_string())
        );

        for (passphrase, new_passphrase) in [
            (None, Some("pass".to_string())),
            (Some("pass".to_string()), Some("pass_new".to_string())),
            (Some("pass".to_string()), None),
        ] {
            let update_private_key_action = UtilsCertificatesAction::UpdatePrivateKey {
                key_id: uuid!("00000000-0000-0000-0000-000000000001"),
                key_name: None,
                passphrase,
                new_passphrase,
            };
            assert!(update_private_key_action.validate().is_ok());
        }

        let get_certificate_templates_actions_with_name =
            |template_name: String| -> anyhow::Result<Vec<UtilsCertificatesAction>> {
                let attributes = MockCertificateAttributes::new(
                    PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    },
                    SignatureAlgorithm::Sha256,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                    Version::One,
                )
                .build();
                Ok(vec![
                    UtilsCertificatesAction::CreateCertificateTemplate {
                        template_name: template_name.clone(),
                        attributes: attributes.clone(),
                    },
                    UtilsCertificatesAction::UpdateCertificateTemplate {
                        template_id: uuid!("00000000-0000-0000-0000-000000000001"),
                        template_name: Some(template_name),
                        attributes: Some(attributes),
                    },
                ])
            };

        for action in get_certificate_templates_actions_with_name("a".repeat(100))? {
            assert!(action.validate().is_ok());
        }

        for action in get_certificate_templates_actions_with_name("".to_string())? {
            assert_eq!(
                action.validate().map_err(|err| err.to_string()),
                Err("Certificate template name cannot be empty.".to_string())
            );
        }

        for action in get_certificate_templates_actions_with_name("a".repeat(101))? {
            assert_eq!(
                action.validate().map_err(|err| err.to_string()),
                Err("Certificate template name cannot be longer than 100 characters.".to_string())
            );
        }

        let update_certificate_template_action =
            UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001"),
                template_name: None,
                attributes: None,
            };
        assert_eq!(
            update_certificate_template_action.validate().map_err(|err| err.to_string()),
            Err("Either new name or attributes should be provided (00000000-0000-0000-0000-000000000001).".to_string())
        );

        Ok(())
    }
}
