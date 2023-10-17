use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, ExportFormat, PrivateKeyAlgorithm,
        UtilsCertificatesActionResult,
    },
};
use anyhow::bail;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesAction {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        template_name: String,
        format: ExportFormat,
        passphrase: Option<String>,
    },
    GetPrivateKeys,
    #[serde(rename_all = "camelCase")]
    CreatePrivateKey {
        key_name: String,
        alg: PrivateKeyAlgorithm,
        passphrase: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    ChangePrivateKeyPassphrase {
        key_name: String,
        passphrase: Option<String>,
        new_passphrase: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    RemovePrivateKey {
        key_name: String,
    },
    #[serde(rename_all = "camelCase")]
    ExportPrivateKey {
        key_name: String,
        format: ExportFormat,
        passphrase: Option<String>,
        export_passphrase: Option<String>,
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

        match self {
            UtilsCertificatesAction::GenerateSelfSignedCertificate { template_name, .. } => {
                if template_name.is_empty() {
                    bail!(SecutilsError::client(
                        "Certificate template name cannot be empty."
                    ));
                }

                if template_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    bail!(SecutilsError::client(format!(
                        "Certificate template name cannot be longer than {} characters.",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    )));
                }
            }
            UtilsCertificatesAction::CreatePrivateKey { key_name: name, .. } => {
                assert_private_key_name(name)?;
            }
            UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                key_name,
                passphrase,
                new_passphrase,
            } => {
                assert_private_key_name(key_name)?;

                if passphrase == new_passphrase {
                    bail!(SecutilsError::client(format!(
                        "New private key passphrase should be different from the current passphrase ({key_name})."
                    )));
                }
            }
            UtilsCertificatesAction::RemovePrivateKey { key_name } => {
                assert_private_key_name(key_name)?;
            }
            UtilsCertificatesAction::ExportPrivateKey { key_name, .. } => {
                assert_private_key_name(key_name)?;
            }
            UtilsCertificatesAction::GetPrivateKeys => {}
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
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name,
                format,
                passphrase,
            } => Ok(
                UtilsCertificatesActionResult::GenerateSelfSignedCertificate(
                    certificates
                        .generate_self_signed_certificate(
                            user.id,
                            &template_name,
                            format,
                            passphrase.as_deref(),
                        )
                        .await?,
                ),
            ),
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
            UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                key_name,
                passphrase,
                new_passphrase,
            } => {
                certificates
                    .change_private_key_passphrase(
                        user.id,
                        &key_name,
                        passphrase.as_deref(),
                        new_passphrase.as_deref(),
                    )
                    .await?;
                Ok(UtilsCertificatesActionResult::ChangePrivateKeyPassphrase)
            }
            UtilsCertificatesAction::ExportPrivateKey {
                key_name,
                passphrase,
                export_passphrase,
                format,
            } => Ok(UtilsCertificatesActionResult::ExportPrivateKey(
                certificates
                    .export_private_key(
                        user.id,
                        &key_name,
                        format,
                        passphrase.as_deref(),
                        export_passphrase.as_deref(),
                    )
                    .await?,
            )),
            UtilsCertificatesAction::RemovePrivateKey { key_name } => {
                certificates.remove_private_key(user.id, &key_name).await?;
                Ok(UtilsCertificatesActionResult::RemovePrivateKey)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ExportFormat, PrivateKeyAlgorithm, PrivateKeySize, UtilsCertificatesAction,
    };
    use insta::assert_debug_snapshot;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "generateSelfSignedCertificate",
    "value": { "templateName": "template", "format": "pem" }
}
          "#
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "template".to_string(),
                format: ExportFormat::Pem,
                passphrase: None,
            }
        );
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "generateSelfSignedCertificate",
    "value": { "templateName": "template", "format": "pkcs12", "passphrase": "phrase" }
}
          "#
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "template".to_string(),
                format: ExportFormat::Pkcs12,
                passphrase: Some("phrase".to_string()),
            }
        );

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
    "type": "changePrivateKeyPassphrase",
    "value": { "keyName": "pk", "passphrase": "phrase", "newPassphrase": "phrase_new" }
}
          "#
            )?,
            UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                key_name: "pk".to_string(),
                passphrase: Some("phrase".to_string()),
                new_passphrase: Some("phrase_new".to_string()),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "changePrivateKeyPassphrase",
    "value": { "keyName": "pk" }
}
          "#
            )?,
            UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                key_name: "pk".to_string(),
                passphrase: None,
                new_passphrase: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "removePrivateKey",
    "value": { "keyName": "pk" }
}
          "#
            )?,
            UtilsCertificatesAction::RemovePrivateKey {
                key_name: "pk".to_string(),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "exportPrivateKey",
    "value": { "keyName": "pk", "format": "pem", "passphrase": "phrase", "exportPassphrase": "phrase_new" }
}
          "#
            )?,
            UtilsCertificatesAction::ExportPrivateKey {
                key_name: "pk".to_string(),
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
    "value": { "keyName": "pk", "format": "pem" }
}
          "#
            )?,
            UtilsCertificatesAction::ExportPrivateKey {
                key_name: "pk".to_string(),
                format: ExportFormat::Pem,
                passphrase: None,
                export_passphrase: None,
            }
        );

        Ok(())
    }

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "a".repeat(100),
            format: ExportFormat::Pem,
            passphrase: None,
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "".to_string(),
            format: ExportFormat::Pem,
            passphrase: None,
        }.validate(), @r###"
        Err(
            "Certificate template name cannot be empty.",
        )
        "###);

        assert_debug_snapshot!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "a".repeat(101),
            format: ExportFormat::Pem,
            passphrase: None,
        }.validate(), @r###"
        Err(
            "Certificate template name cannot be longer than 100 characters.",
        )
        "###);

        let get_actions_with_name = |key_name: String| {
            vec![
                UtilsCertificatesAction::CreatePrivateKey {
                    key_name: key_name.clone(),
                    alg: PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    },
                    passphrase: Some("phrase".to_string()),
                },
                UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                    key_name: key_name.clone(),
                    passphrase: Some("pass".to_string()),
                    new_passphrase: Some("pass_new".to_string()),
                },
                UtilsCertificatesAction::ExportPrivateKey {
                    key_name: key_name.clone(),
                    format: ExportFormat::Pem,
                    passphrase: None,
                    export_passphrase: None,
                },
                UtilsCertificatesAction::RemovePrivateKey {
                    key_name: key_name.clone(),
                },
            ]
        };

        for action in get_actions_with_name("a".repeat(100)) {
            assert!(action.validate().is_ok());
        }

        for action in get_actions_with_name("".to_string()) {
            assert_eq!(
                action.validate().map_err(|err| err.to_string()),
                Err("Private key name cannot be empty.".to_string())
            );
        }

        for action in get_actions_with_name("a".repeat(101)) {
            assert_eq!(
                action.validate().map_err(|err| err.to_string()),
                Err("Private key name cannot be longer than 100 characters.".to_string())
            );
        }

        for (passphrase, new_passphrase) in [
            (None, None),
            (Some("pass".to_string()), Some("pass".to_string())),
        ] {
            let change_password_action = UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                key_name: "pk".to_string(),
                passphrase,
                new_passphrase,
            };
            assert_eq!(
                change_password_action.validate().map_err(|err| err.to_string()),
                Err("New private key passphrase should be different from the current passphrase (pk).".to_string())
            );
        }

        for (passphrase, new_passphrase) in [
            (None, Some("pass".to_string())),
            (Some("pass".to_string()), Some("pass_new".to_string())),
            (Some("pass".to_string()), None),
        ] {
            let change_password_action = UtilsCertificatesAction::ChangePrivateKeyPassphrase {
                key_name: "pk".to_string(),
                passphrase,
                new_passphrase,
            };
            assert!(change_password_action.validate().is_ok());
        }

        Ok(())
    }
}
