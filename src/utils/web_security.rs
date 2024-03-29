mod api_ext;
mod csp;
mod database_ext;

pub use self::{
    api_ext::ContentSecurityPolicyContent,
    csp::{
        ContentSecurityPolicy, ContentSecurityPolicyDirective,
        ContentSecurityPolicyRequireTrustedTypesForDirectiveValue,
        ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
        ContentSecurityPolicyTrustedTypesDirectiveValue, ContentSecurityPolicyWebrtcDirectiveValue,
    },
};
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

fn extract_params<T: for<'de> Deserialize<'de>>(
    params: Option<UtilsActionParams>,
) -> anyhow::Result<T> {
    params
        .ok_or_else(|| SecutilsError::client("Missing required action parameters."))?
        .into_inner()
}

pub async fn web_security_handle_action<DR: DnsResolver, ET: EmailTransport>(
    user: User,
    api: &Api<DR, ET>,
    action: UtilsAction,
    resource: UtilsResource,
    params: Option<UtilsActionParams>,
) -> anyhow::Result<UtilsActionResult> {
    let web_security = api.web_security();
    match (resource, action) {
        (UtilsResource::WebSecurityContentSecurityPolicies, UtilsAction::Get { resource_id }) => {
            let users = api.users();
            let Some(policy) = web_security
                .get_content_security_policy(user.id, resource_id)
                .await?
            else {
                return Ok(UtilsActionResult::empty());
            };

            UtilsActionResult::json(json!({
                "policy": policy,
                "userShare": users
                    .get_user_share_by_resource(
                        user.id,
                        &SharedResource::content_security_policy(resource_id),
                    )
                    .await?
                    .map(ClientUserShare::from),
            }))
        }
        (UtilsResource::WebSecurityContentSecurityPolicies, UtilsAction::List) => {
            UtilsActionResult::json(web_security.get_content_security_policies(user.id).await?)
        }
        (UtilsResource::WebSecurityContentSecurityPolicies, UtilsAction::Create) => {
            UtilsActionResult::json(
                web_security
                    .create_content_security_policy(user.id, extract_params(params)?)
                    .await?,
            )
        }
        (
            UtilsResource::WebSecurityContentSecurityPolicies,
            UtilsAction::Update { resource_id },
        ) => {
            web_security
                .update_content_security_policy(user.id, resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebSecurityContentSecurityPolicies,
            UtilsAction::Delete { resource_id },
        ) => {
            web_security
                .remove_content_security_policy(user.id, resource_id)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebSecurityContentSecurityPolicies,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize,
            },
        ) => UtilsActionResult::json(
            web_security
                .serialize_content_security_policy(user.id, resource_id, extract_params(params)?)
                .await?,
        ),
        (UtilsResource::WebSecurityContentSecurityPolicies, UtilsAction::Share { resource_id }) => {
            UtilsActionResult::json(
                web_security
                    .share_content_security_policy(user.id, resource_id)
                    .await
                    .map(ClientUserShare::from)?,
            )
        }
        (
            UtilsResource::WebSecurityContentSecurityPolicies,
            UtilsAction::Unshare { resource_id },
        ) => UtilsActionResult::json(
            web_security
                .unshare_content_security_policy(user.id, resource_id)
                .await
                .map(|user_share| user_share.map(ClientUserShare::from))?,
        ),
        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        tests::{mock_api, mock_user},
        users::{SharedResource, UserShareId},
        utils::{
            web_security::{
                api_ext::ContentSecurityPoliciesCreateParams, web_security_handle_action,
                ContentSecurityPolicy, ContentSecurityPolicyContent,
                ContentSecurityPolicyDirective, ContentSecurityPolicySandboxDirectiveValue,
                ContentSecurityPolicyTrustedTypesDirectiveValue,
            },
            UtilsAction, UtilsActionParams, UtilsResource, UtilsResourceOperation,
        },
    };
    use serde::Deserialize;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::uuid;

    fn get_mock_directives() -> anyhow::Result<Vec<ContentSecurityPolicyDirective>> {
        Ok(vec![
            ContentSecurityPolicyDirective::UpgradeInsecureRequests,
            ContentSecurityPolicyDirective::DefaultSrc(
                ["'self'".to_string(), "https://secutils.dev".to_string()]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::TrustedTypes(
                [ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::Sandbox(
                [
                    ContentSecurityPolicySandboxDirectiveValue::AllowForms,
                    ContentSecurityPolicySandboxDirectiveValue::AllowPopups,
                ]
                .into_iter()
                .collect(),
            ),
            ContentSecurityPolicyDirective::ReportUri(
                ["https://secutils.dev/report".to_string()]
                    .into_iter()
                    .collect(),
            ),
        ])
    }

    #[sqlx::test]
    async fn can_list_content_security_policies(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = api.web_security();
        web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;
        web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp-2".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let serialized_policies = web_security_handle_action(
            mock_user,
            &api,
            UtilsAction::List,
            UtilsResource::WebSecurityContentSecurityPolicies,
            None,
        )
        .await?;

        let policies = serde_json::from_value::<Vec<ContentSecurityPolicy>>(
            serialized_policies.into_inner().unwrap(),
        )?;
        assert_eq!(policies.len(), 2);

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_content_security_policy(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = api.web_security();
        let policy_original = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let serialized_policy = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Get {
                resource_id: policy_original.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
            None,
        )
        .await?;

        #[derive(Deserialize)]
        struct UserShareWrapper {
            id: UserShareId,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ContentSecurityPolicyWrapper {
            policy: ContentSecurityPolicy,
            user_share: Option<UserShareWrapper>,
        }

        let policy = serde_json::from_value::<ContentSecurityPolicyWrapper>(
            serialized_policy.into_inner().unwrap(),
        )?;
        assert_eq!(policy_original.id, policy.policy.id);
        assert_eq!(policy_original.name, policy.policy.name);
        assert!(policy.user_share.is_none());

        // Share policy.
        let user_share = web_security
            .share_content_security_policy(mock_user.id, policy_original.id)
            .await?;
        let serialized_policy = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Get {
                resource_id: policy_original.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
            None,
        )
        .await?;
        let policy = serde_json::from_value::<ContentSecurityPolicyWrapper>(
            serialized_policy.into_inner().unwrap(),
        )?;
        assert_eq!(policy_original.id, policy.policy.id);
        assert_eq!(policy_original.name, policy.policy.name);
        assert_eq!(policy.user_share.unwrap().id, user_share.id);

        let empty_result = web_security_handle_action(
            mock_user,
            &api,
            UtilsAction::Get {
                resource_id: uuid!("00000000-0000-0000-0000-000000000000"),
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
            None,
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_create_content_security_policy(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let serialized_policy = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebSecurityContentSecurityPolicies,
            Some(UtilsActionParams::json(json!({
                "name": "csp",
                "content": { "type": "directives", "value": get_mock_directives()? }
            }))),
        )
        .await?;
        let policy = serde_json::from_value::<ContentSecurityPolicy>(
            serialized_policy.into_inner().unwrap(),
        )?;
        assert_eq!(policy.name, "csp");
        assert_eq!(policy.directives, get_mock_directives()?);

        let policy = api
            .web_security()
            .get_content_security_policy(mock_user.id, policy.id)
            .await?
            .unwrap();
        assert_eq!(policy.name, "csp");

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_content_security_policy(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = api.web_security();
        let policy_original = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let empty_result = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: policy_original.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
            Some(UtilsActionParams::json(json!({
                "name": "csp-new",
            }))),
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        let policy = api
            .web_security()
            .get_content_security_policy(mock_user.id, policy_original.id)
            .await?
            .unwrap();
        assert_eq!(policy.name, "csp-new");

        Ok(())
    }

    #[sqlx::test]
    async fn can_delete_content_security_policy(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = api.web_security();
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let empty_result = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: policy.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
            None,
        )
        .await?;
        assert!(empty_result.into_inner().is_none());

        assert!(api
            .web_security()
            .get_content_security_policy(mock_user.id, policy.id)
            .await?
            .is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_serialize_content_security_policy(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = api.web_security();
        let policy_original = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let serialize_result = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: policy_original.id,
                operation: UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
            Some(UtilsActionParams::json(json!({
                "source": "enforcingHeader",
            }))),
        )
        .await?;

        let deserialized_result =
            serde_json::from_value::<String>(serialize_result.into_inner().unwrap())?;
        assert_eq!(deserialized_result, "upgrade-insecure-requests; default-src 'self' https://secutils.dev; trusted-types 'allow-duplicates'; sandbox allow-forms allow-popups; report-uri https://secutils.dev/report");

        Ok(())
    }

    #[sqlx::test]
    async fn can_share_and_unshare_content_security_policy(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = api.web_security();
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "csp".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let serialized_user_share = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Share {
                resource_id: policy.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
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
            SharedResource::ContentSecurityPolicy {
                policy_id: policy.id
            }
        );

        let serialized_user_share = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Unshare {
                resource_id: policy.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
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

        let serialized_user_share = web_security_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Unshare {
                resource_id: policy.id,
            },
            UtilsResource::WebSecurityContentSecurityPolicies,
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
