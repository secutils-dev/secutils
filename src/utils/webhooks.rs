mod api_ext;
mod database_ext;
mod responders;

pub use self::{
    api_ext::RespondersRequestCreateParams,
    responders::{
        Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderRequest,
        ResponderRequestHeaders, ResponderScriptContext, ResponderScriptResult, ResponderSettings,
        ResponderStats,
    },
};
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

fn extract_params<T: for<'de> Deserialize<'de>>(
    params: Option<UtilsActionParams>,
) -> anyhow::Result<T> {
    params
        .ok_or_else(|| SecutilsError::client("Missing required action parameters."))?
        .into_inner()
}

pub async fn webhooks_handle_action<DR: DnsResolver, ET: EmailTransport>(
    user: User,
    api: &Api<DR, ET>,
    action: UtilsAction,
    resource: UtilsResource,
    params: Option<UtilsActionParams>,
) -> anyhow::Result<UtilsActionResult> {
    let webhooks = api.webhooks(&user);
    match (resource, action) {
        (UtilsResource::WebhooksResponders, UtilsAction::List) => {
            UtilsActionResult::json(webhooks.get_responders().await?)
        }
        (UtilsResource::WebhooksResponders, UtilsAction::Create) => {
            UtilsActionResult::json(webhooks.create_responder(extract_params(params)?).await?)
        }
        (UtilsResource::WebhooksResponders, UtilsAction::Update { resource_id }) => {
            webhooks
                .update_responder(resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::WebhooksResponders, UtilsAction::Delete { resource_id }) => {
            webhooks.remove_responder(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebhooksResponders,
            UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebhooksRespondersGetHistory,
            },
        ) => UtilsActionResult::json(webhooks.get_responder_requests(resource_id).await?),
        (
            UtilsResource::WebhooksResponders,
            UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebhooksRespondersClearHistory,
            },
        ) => {
            webhooks.clear_responder_requests(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebhooksResponders,
            UtilsAction::Execute {
                operation: UtilsResourceOperation::WebhooksRespondersGetStats,
                ..
            },
        ) => UtilsActionResult::json(webhooks.get_responders_stats().await?),
        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    pub use crate::utils::webhooks::api_ext::{RespondersCreateParams, RespondersUpdateParams};
    use crate::{
        tests::{mock_api, mock_user},
        utils::{
            UtilsAction, UtilsActionParams, UtilsResource, UtilsResourceOperation,
            webhooks::{
                Responder, ResponderLocation, ResponderMethod, ResponderPathType,
                ResponderSettings, RespondersRequestCreateParams, webhooks_handle_action,
            },
        },
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use sqlx::PgPool;
    use std::borrow::Cow;
    use time::OffsetDateTime;
    use uuid::Uuid;

    pub struct MockResponderBuilder {
        responder: Responder,
    }

    impl MockResponderBuilder {
        pub fn create(id: Uuid, name: &str, path: &str) -> anyhow::Result<Self> {
            Ok(Self {
                responder: Responder {
                    id,
                    name: name.to_string(),
                    location: ResponderLocation {
                        path_type: ResponderPathType::Exact,
                        path: path.to_string(),
                        subdomain_prefix: None,
                    },
                    method: ResponderMethod::Any,
                    enabled: true,
                    settings: ResponderSettings {
                        requests_to_track: 0,
                        status_code: 200,
                        body: None,
                        headers: None,
                        script: None,
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            })
        }

        pub fn with_method(mut self, method: ResponderMethod) -> Self {
            self.responder.method = method;
            self
        }

        pub fn with_body(mut self, body: &str) -> Self {
            self.responder.settings.body = Some(body.to_string());
            self
        }

        pub fn with_location(mut self, location: ResponderLocation) -> Self {
            self.responder.location = location;
            self
        }

        pub fn build(self) -> Responder {
            self.responder
        }
    }

    #[sqlx::test]
    async fn properly_handles_responders_list_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;
        assert_json_snapshot!(action_result.into_inner().unwrap(), @"[]");

        let webhooks = api.webhooks(&mock_user);
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
            })
            .await?;
        let responder_two = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_two".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: false,
                settings: responder_one.settings.clone(),
            })
            .await?;
        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;
        let mut settings = insta::Settings::clone_current();
        for responder in [responder_one, responder_two] {
            settings.add_filter(&responder.id.to_string(), "[UUID]");
            settings.add_filter(
                &responder.created_at.unix_timestamp().to_string(),
                "[TIMESTAMP]",
            );
        }
        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"id\":\"[UUID]\",\"name\":\"name_one\",\"location\":{\"pathType\":\"=\",\"path\":\"/\"},\"method\":\"GET\",\"enabled\":true,\"settings\":{\"requestsToTrack\":3,\"statusCode\":200},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]},{\"id\":\"[UUID]\",\"name\":\"name_two\",\"location\":{\"pathType\":\"=\",\"path\":\"/path\"},\"method\":\"GET\",\"enabled\":false,\"settings\":{\"requestsToTrack\":3,\"statusCode\":200},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]}]""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_responders_create_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebhooksResponders,
            Some(UtilsActionParams::json(json!({
                "name": "name_one",
                "location": {
                    "pathType": "^",
                    "path": "/",
                    "subdomainPrefix": "sub"
                },
                "method": "GET",
                "enabled": true,
                "settings": ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                }
            }))),
        )
        .await?;

        // Extract responder to make sure it has been saved.
        let webhooks = api.webhooks(&mock_user);
        let responder = webhooks.get_responders().await?.pop().unwrap();
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&responder.id.to_string(), "[UUID]");
        settings.add_filter(
            &responder.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );

        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""{\"id\":\"[UUID]\",\"name\":\"name_one\",\"location\":{\"pathType\":\"^\",\"path\":\"/\",\"subdomainPrefix\":\"sub\"},\"method\":\"GET\",\"enabled\":true,\"settings\":{\"requestsToTrack\":3,\"statusCode\":200},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]}""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_responders_update_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
            })
            .await?;

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: responder.id,
            },
            UtilsResource::WebhooksResponders,
            Some(UtilsActionParams::json(json!({
                "name": "name_one_updated",
                "location": {
                    "pathType": "^",
                    "path": "/path",
                    "subdomainPrefix": "sub"
                },
                "method": "GET",
                "enabled": false,
                "settings": ResponderSettings {
                    requests_to_track: 10,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                }
            }))),
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract responder to make sure it has been updated.
        let updated_responder = webhooks.get_responder(responder.id).await?.unwrap();
        assert_eq!(
            updated_responder,
            Responder {
                id: responder.id,
                name: "name_one_updated".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Prefix,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("sub".to_string()),
                },
                method: ResponderMethod::Get,
                enabled: false,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
                created_at: responder.created_at,
                updated_at: responder.updated_at
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_responder_delete_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
            })
            .await?;

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: responder.id,
            },
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract responder to make sure it has been updated.
        let deleted_responder = webhooks.get_responder(responder.id).await?;
        assert!(deleted_responder.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_get_history_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert responders and requests.
        let webhooks = api.webhooks(&mock_user);
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
            })
            .await?;
        let request_one = webhooks
            .create_responder_request(
                responder.id,
                RespondersRequestCreateParams {
                    client_address: None,
                    method: Cow::Borrowed("POST"),
                    headers: None,
                    url: Cow::Borrowed("/?query=value"),
                    body: None,
                },
            )
            .await?
            .unwrap();
        let request_two = webhooks
            .create_responder_request(
                responder.id,
                RespondersRequestCreateParams {
                    client_address: None,
                    method: Cow::Borrowed("POST"),
                    headers: None,
                    url: Cow::Borrowed("/?query=other-value"),
                    body: None,
                },
            )
            .await?
            .unwrap();

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(responder.id),
                operation: UtilsResourceOperation::WebhooksRespondersGetHistory,
            },
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;

        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&request_one.id.to_string(), "[UUID-1]");
        settings.add_filter(&request_two.id.to_string(), "[UUID-2]");
        settings.add_filter(
            &request_one.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP-1]",
        );
        settings.add_filter(
            &request_two.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP-2]",
        );

        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"id\":\"[UUID-1]\",\"method\":\"POST\",\"url\":\"/?query=value\",\"createdAt\":[TIMESTAMP-1]},{\"id\":\"[UUID-2]\",\"method\":\"POST\",\"url\":\"/?query=other-value\",\"createdAt\":[TIMESTAMP-1]}]""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_clear_history_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert responder and requests.
        let webhooks = api.webhooks(&mock_user);
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
            })
            .await?;
        webhooks
            .create_responder_request(
                responder.id,
                RespondersRequestCreateParams {
                    client_address: None,
                    method: Cow::Borrowed("POST"),
                    headers: None,
                    url: Cow::Borrowed("/?query=value"),
                    body: None,
                },
            )
            .await?;
        webhooks
            .create_responder_request(
                responder.id,
                RespondersRequestCreateParams {
                    client_address: None,
                    method: Cow::Borrowed("POST"),
                    headers: None,
                    url: Cow::Borrowed("/?query=other-value"),
                    body: None,
                },
            )
            .await?;

        assert_eq!(
            webhooks.get_responder_requests(responder.id).await?.len(),
            2
        );

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(responder.id),
                operation: UtilsResourceOperation::WebhooksRespondersClearHistory,
            },
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        assert!(
            webhooks
                .get_responder_requests(responder.id)
                .await?
                .is_empty()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_get_stats_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert responders and requests.
        let webhooks = api.webhooks(&mock_user);
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
            })
            .await?;
        let request_one = webhooks
            .create_responder_request(
                responder.id,
                RespondersRequestCreateParams {
                    client_address: None,
                    method: Cow::Borrowed("POST"),
                    headers: None,
                    url: Cow::Borrowed("/?query=value"),
                    body: None,
                },
            )
            .await?
            .unwrap();

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: None,
                operation: UtilsResourceOperation::WebhooksRespondersGetStats,
            },
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;

        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&responder.id.to_string(), "[UUID]");
        settings.add_filter(
            &request_one.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );

        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"responderId\":\"[UUID]\",\"requestCount\":1,\"lastRequestedAt\":[TIMESTAMP]}]""###
            );
        });

        Ok(())
    }
}
