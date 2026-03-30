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
    users::User,
    utils::{
        UtilsAction, UtilsActionParams, UtilsActionResult, UtilsResource, UtilsResourceOperation,
    },
};
use serde::Deserialize;

pub use self::{
    api_ext::{
        TemplatesCreateParams, TemplatesFetchCertificatesParams, TemplatesGenerateParams,
        TemplatesUpdateParams,
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
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::CertificatesPrivateKeyExport,
            },
        ) => UtilsActionResult::json(
            certificates
                .export_private_key(user.id, resource_id, extract_params(params)?)
                .await?,
        ),
        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    pub use super::certificate_templates::tests::*;
    use super::certificates_handle_action;
    pub use crate::utils::certificates::api_ext::{PrivateKeysCreateParams, TemplatesCreateParams};
    use crate::{
        tests::{mock_api, mock_user},
        utils::{
            UtilsAction, UtilsActionParams, UtilsResource, UtilsResourceOperation,
            certificates::{PrivateKey, PrivateKeyAlgorithm},
        },
    };
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_list_private_keys(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

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
                    tag_ids: vec![],
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
                    tag_ids: vec![],
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

    #[sqlx::test]
    async fn can_retrieve_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

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
                    tag_ids: vec![],
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

    #[sqlx::test]
    async fn can_create_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

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

    #[sqlx::test]
    async fn can_update_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

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
                    tag_ids: vec![],
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

    #[sqlx::test]
    async fn can_delete_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

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
                    tag_ids: vec![],
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

        assert!(
            api.certificates()
                .get_private_key(mock_user.id, private_key.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_export_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

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
                    tag_ids: vec![],
                },
            )
            .await?;

        let export_result = certificates_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(private_key_original.id),
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

    // Certificate template dispatch tests were removed since templates are now served by
    // dedicated routes in src/server/handlers/certificate_templates.rs. The underlying
    // API logic is tested in src/utils/certificates/api_ext.rs.
}
