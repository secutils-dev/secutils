mod certificate_templates;
mod database_ext;
mod export_format;
mod private_keys;
mod x509;

mod api_ext;

use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::{ClientUserShare, SharedResource, User},
    utils::{
        UtilsAction, UtilsActionParams, UtilsActionResult, UtilsResource, UtilsResourceOperation,
    },
};
use serde::Deserialize;
use serde_json::json;

pub use self::{
    api_ext::{
        CertificatesApi, PrivateKeysCreateParams, PrivateKeysExportParams, PrivateKeysUpdateParams,
        TemplatesCreateParams, TemplatesGenerateParams, TemplatesUpdateParams,
    },
    certificate_templates::{CertificateAttributes, CertificateTemplate},
    export_format::ExportFormat,
    private_keys::{PrivateKey, PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize},
    x509::{ExtendedKeyUsage, KeyUsage, SignatureAlgorithm, Version},
};

fn extract_params<T: for<'de> Deserialize<'de>>(
    params: Option<UtilsActionParams>,
) -> anyhow::Result<T> {
    params
        .ok_or_else(|| SecutilsError::client("Missing required action parameters."))?
        .into_inner()
}

pub async fn certificates_handle_action<DR: DnsResolver, ET: EmailTransport>(
    user: User,
    api: &Api<DR, ET>,
    action: UtilsAction,
    resource: UtilsResource,
    params: Option<UtilsActionParams>,
) -> anyhow::Result<UtilsActionResult> {
    let certificates = api.certificates();
    match (resource, action) {
        (UtilsResource::CertificatesPrivateKeys, UtilsAction::List) => {
            UtilsActionResult::json(certificates.get_private_keys(user.id).await?)
        }
        (UtilsResource::CertificatesPrivateKeys, UtilsAction::Get { resource_id }) => {
            if let Some(private_key) = certificates.get_private_key(user.id, resource_id).await? {
                UtilsActionResult::json(private_key)
            } else {
                Ok(UtilsActionResult::empty())
            }
        }
        (UtilsResource::CertificatesPrivateKeys, UtilsAction::Create) => UtilsActionResult::json(
            certificates
                .create_private_key(user.id, extract_params(params)?)
                .await?,
        ),
        (UtilsResource::CertificatesPrivateKeys, UtilsAction::Update { resource_id }) => {
            certificates
                .update_private_key(user.id, resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::CertificatesPrivateKeys, UtilsAction::Delete { resource_id }) => {
            certificates
                .remove_private_key(user.id, resource_id)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::CertificatesPrivateKeys,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::CertificatesPrivateKeyExport,
            },
        ) => UtilsActionResult::json(
            certificates
                .export_private_key(user.id, resource_id, extract_params(params)?)
                .await?,
        ),
        // Certificate templates.
        (UtilsResource::CertificatesTemplates, UtilsAction::List) => {
            UtilsActionResult::json(certificates.get_certificate_templates(user.id).await?)
        }
        (UtilsResource::CertificatesTemplates, UtilsAction::Get { resource_id }) => {
            let users = api.users();
            let Some(template) = certificates
                .get_certificate_template(user.id, resource_id)
                .await?
            else {
                return Ok(UtilsActionResult::empty());
            };

            UtilsActionResult::json(json!({
                "template": template,
                "userShare": users
                    .get_user_share_by_resource(
                        user.id,
                        &SharedResource::certificate_template(resource_id),
                    )
                    .await?
                    .map(ClientUserShare::from),
            }))
        }
        (UtilsResource::CertificatesTemplates, UtilsAction::Create) => UtilsActionResult::json(
            certificates
                .create_certificate_template(user.id, extract_params(params)?)
                .await?,
        ),
        (UtilsResource::CertificatesTemplates, UtilsAction::Update { resource_id }) => {
            certificates
                .update_certificate_template(user.id, resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::CertificatesTemplates, UtilsAction::Delete { resource_id }) => {
            certificates
                .remove_certificate_template(user.id, resource_id)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::CertificatesTemplates,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::CertificatesTemplateGenerate,
            },
        ) => UtilsActionResult::json(
            certificates
                .generate_self_signed_certificate(user.id, resource_id, extract_params(params)?)
                .await?,
        ),
        (
            UtilsResource::CertificatesTemplates,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::CertificatesTemplateShare,
            },
        ) => UtilsActionResult::json(
            certificates
                .share_certificate_template(user.id, resource_id)
                .await
                .map(ClientUserShare::from)?,
        ),
        (
            UtilsResource::CertificatesTemplates,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::CertificatesTemplateUnshare,
            },
        ) => UtilsActionResult::json(
            certificates
                .unshare_certificate_template(user.id, resource_id)
                .await
                .map(|user_share| user_share.map(ClientUserShare::from))?,
        ),

        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    pub use super::certificate_templates::tests::*;
    use super::certificates_handle_action;
    use crate::{
        tests::{mock_api, mock_user},
        users::{SharedResource, UserShareId},
        utils::{
            CertificateAttributes, CertificateTemplate, ExtendedKeyUsage, KeyUsage, PrivateKey,
            PrivateKeyAlgorithm, PrivateKeySize, PrivateKeysCreateParams, SignatureAlgorithm,
            TemplatesCreateParams, UtilsAction, UtilsActionParams, UtilsResource,
            UtilsResourceOperation, Version,
        },
    };
    use serde::Deserialize;
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::uuid;

    fn get_mock_certificate_attributes() -> anyhow::Result<CertificateAttributes> {
        Ok(CertificateAttributes {
            common_name: Some("my-common-name".to_string()),
            country: Some("DE".to_string()),
            state_or_province: Some("BE".to_string()),
            locality: None,
            organization: None,
            organizational_unit: None,
            key_algorithm: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024,
            },
            signature_algorithm: SignatureAlgorithm::Sha256,
            not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
            not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
            version: Version::One,
            is_ca: true,
            key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
            extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
        })
    }

    #[actix_rt::test]
    async fn can_list_private_keys() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;
        certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk-2".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        let serialized_private_keys = certificates_handle_action(
            mock_user,
            &api,
            UtilsAction::List,
            UtilsResource::CertificatesPrivateKeys,
            None,
        )
        .await?;

        let private_keys = serde_json::from_value::<Vec<PrivateKey>>(
            serialized_private_keys.into_inner().unwrap(),
        )?;
        assert_eq!(private_keys.len(), 2);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key_original = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        let serialized_private_key = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Get {
                resource_id: private_key_original.id,
            },
            UtilsResource::CertificatesPrivateKeys,
            None,
        )
        .await?;

        let private_key =
            serde_json::from_value::<PrivateKey>(serialized_private_key.into_inner().unwrap())?;
        assert_eq!(private_key_original.id, private_key.id);
        assert_eq!(private_key_original.name, private_key.name);

        let empty_result = certificates_handle_action(
            mock_user,
            &api,
            UtilsAction::Get {
                resource_id: uuid!("00000000-0000-0000-0000-000000000000"),
            },
            UtilsResource::CertificatesPrivateKeys,
            None,
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_create_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let serialized_private_key = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::CertificatesPrivateKeys,
            Some(UtilsActionParams::json(json!({
                "keyName": "pk",
                "alg": { "keyType": "ed25519" },
            }))),
        )
        .await?;
        let private_key =
            serde_json::from_value::<PrivateKey>(serialized_private_key.into_inner().unwrap())?;
        assert_eq!(private_key.name, "pk");
        assert!(matches!(private_key.alg, PrivateKeyAlgorithm::Ed25519));

        let private_key = api
            .certificates()
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert_eq!(private_key.name, "pk");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key_original = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        let empty_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: private_key_original.id,
            },
            UtilsResource::CertificatesPrivateKeys,
            Some(UtilsActionParams::json(json!({
                "keyName": "pk-new",
            }))),
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        let private_key = api
            .certificates()
            .get_private_key(mock_user.id, private_key_original.id)
            .await?
            .unwrap();
        assert_eq!(private_key.name, "pk-new");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_delete_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        let empty_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: private_key.id,
            },
            UtilsResource::CertificatesPrivateKeys,
            None,
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        assert!(api
            .certificates()
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_export_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key_original = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        let export_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: private_key_original.id,
                operation: UtilsResourceOperation::CertificatesPrivateKeyExport,
            },
            UtilsResource::CertificatesPrivateKeys,
            Some(UtilsActionParams::json(json!({
                "format": "pem",
            }))),
        )
        .await?;

        let export_result = serde_json::from_value::<Vec<u8>>(export_result.into_inner().unwrap())?;
        assert_eq!(export_result.len(), 119);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_list_templates() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct-2".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let serialized_templates = certificates_handle_action(
            mock_user,
            &api,
            UtilsAction::List,
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;

        let templates = serde_json::from_value::<Vec<CertificateTemplate>>(
            serialized_templates.into_inner().unwrap(),
        )?;
        assert_eq!(templates.len(), 2);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let template_original = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let serialized_template = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Get {
                resource_id: template_original.id,
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;

        #[derive(Deserialize)]
        struct UserShareWrapper {
            id: UserShareId,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TemplateWrapper {
            template: CertificateTemplate,
            user_share: Option<UserShareWrapper>,
        }

        let template =
            serde_json::from_value::<TemplateWrapper>(serialized_template.into_inner().unwrap())?;
        assert_eq!(template_original.id, template.template.id);
        assert_eq!(template_original.name, template.template.name);
        assert!(template.user_share.is_none());

        // Share template.
        let user_share = certificates
            .share_certificate_template(mock_user.id, template_original.id)
            .await?;
        let serialized_template = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Get {
                resource_id: template_original.id,
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;
        let template =
            serde_json::from_value::<TemplateWrapper>(serialized_template.into_inner().unwrap())?;
        assert_eq!(template_original.id, template.template.id);
        assert_eq!(template_original.name, template.template.name);
        assert_eq!(template.user_share.unwrap().id, user_share.id);

        let empty_result = certificates_handle_action(
            mock_user,
            &api,
            UtilsAction::Get {
                resource_id: uuid!("00000000-0000-0000-0000-000000000000"),
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_create_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let serialized_template = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::CertificatesTemplates,
            Some(UtilsActionParams::json(json!({
                "templateName": "ct",
                "attributes": serde_json::to_value(get_mock_certificate_attributes()?)?,
            }))),
        )
        .await?;
        let template = serde_json::from_value::<CertificateTemplate>(
            serialized_template.into_inner().unwrap(),
        )?;
        assert_eq!(template.name, "ct");
        assert_eq!(template.attributes, get_mock_certificate_attributes()?);

        let template = api
            .certificates()
            .get_certificate_template(mock_user.id, template.id)
            .await?
            .unwrap();
        assert_eq!(template.name, "ct");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let template_original = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let empty_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: template_original.id,
            },
            UtilsResource::CertificatesTemplates,
            Some(UtilsActionParams::json(json!({
                "templateName": "ct-new",
            }))),
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        let template = api
            .certificates()
            .get_certificate_template(mock_user.id, template_original.id)
            .await?
            .unwrap();
        assert_eq!(template.name, "ct-new");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_delete_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let empty_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: template.id,
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        assert!(api
            .certificates()
            .get_certificate_template(mock_user.id, template.id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_generate_key_pair() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let template_original = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let generate_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: template_original.id,
                operation: UtilsResourceOperation::CertificatesTemplateGenerate,
            },
            UtilsResource::CertificatesTemplates,
            Some(UtilsActionParams::json(json!({
                "format": "pem",
            }))),
        )
        .await?;

        let generate_result =
            serde_json::from_value::<Vec<u8>>(generate_result.into_inner().unwrap())?;
        assert!(generate_result.len() > 1000);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_share_and_unshare_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let serialized_user_share = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: template.id,
                operation: UtilsResourceOperation::CertificatesTemplateShare,
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;

        #[derive(Deserialize)]
        struct UserShareWrapper {
            id: UserShareId,
        }

        let UserShareWrapper { id: user_share_id } = serde_json::from_value::<UserShareWrapper>(
            serialized_user_share.into_inner().unwrap(),
        )?;
        assert_eq!(
            api.users()
                .get_user_share(user_share_id)
                .await?
                .unwrap()
                .resource,
            SharedResource::CertificateTemplate {
                template_id: template.id
            }
        );

        let serialized_user_share = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: template.id,
                operation: UtilsResourceOperation::CertificatesTemplateUnshare,
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;

        let UserShareWrapper {
            id: user_unshare_id,
        } = serde_json::from_value::<UserShareWrapper>(
            serialized_user_share.into_inner().unwrap(),
        )?;
        assert_eq!(user_unshare_id, user_share_id);
        assert!(api.users().get_user_share(user_share_id).await?.is_none());

        let serialized_user_share = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: template.id,
                operation: UtilsResourceOperation::CertificatesTemplateUnshare,
            },
            UtilsResource::CertificatesTemplates,
            None,
        )
        .await?;

        let user_unshare = serde_json::from_value::<Option<UserShareWrapper>>(
            serialized_user_share.into_inner().unwrap(),
        )?;
        assert!(user_unshare.is_none());
        assert!(api.users().get_user_share(user_share_id).await?.is_none());

        Ok(())
    }
}
