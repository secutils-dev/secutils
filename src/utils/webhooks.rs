mod api_ext;
mod database_ext;
mod responders;

pub use self::{
    api_ext::{
        RespondersCreateParams, RespondersRequestCreateParams, RespondersUpdateParams,
        WebhooksApiExt,
    },
    responders::{
        Responder, ResponderMethod, ResponderRequest, ResponderRequestHeaders,
        ResponderScriptContext, ResponderScriptResult, ResponderSettings,
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
    let webhooks = api.webhooks();
    match (resource, action) {
        (UtilsResource::WebhooksResponders, UtilsAction::List) => {
            UtilsActionResult::json(webhooks.get_responders(user.id).await?)
        }
        (UtilsResource::WebhooksResponders, UtilsAction::Create) => UtilsActionResult::json(
            webhooks
                .create_responder(user.id, extract_params(params)?)
                .await?,
        ),
        (UtilsResource::WebhooksResponders, UtilsAction::Update { resource_id }) => {
            webhooks
                .update_responder(user.id, resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::WebhooksResponders, UtilsAction::Delete { resource_id }) => {
            webhooks.remove_responder(user.id, resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebhooksResponders,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::WebhooksRespondersGetHistory,
            },
        ) => UtilsActionResult::json(
            webhooks
                .get_responder_requests(user.id, resource_id)
                .await?,
        ),
        (
            UtilsResource::WebhooksResponders,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::WebhooksRespondersClearHistory,
            },
        ) => {
            webhooks
                .clear_responder_requests(user.id, resource_id)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        tests::{mock_api, mock_user},
        utils::{
            webhooks_handle_action, Responder, ResponderMethod, ResponderSettings,
            RespondersCreateParams, RespondersRequestCreateParams, UtilsAction, UtilsActionParams,
            UtilsResource, UtilsResourceOperation,
        },
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
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
                    path: path.to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 0,
                        status_code: 200,
                        body: None,
                        headers: None,
                        script: None,
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
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
        pub fn build(self) -> Responder {
            self.responder
        }
    }

    #[tokio::test]
    async fn properly_handles_responders_list_operation() -> anyhow::Result<()> {
        let api = mock_api().await?;
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

        let webhooks = api.webhooks();
        let responder_one = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Get,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        script: None,
                        status_code: 200,
                        body: None,
                        headers: None,
                    },
                },
            )
            .await?;
        let responder_two = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_two".to_string(),
                    path: "/path".to_string(),
                    method: ResponderMethod::Get,
                    settings: responder_one.settings.clone(),
                },
            )
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
                @r###""[{\"id\":\"[UUID]\",\"name\":\"name_one\",\"path\":\"/\",\"method\":\"GET\",\"settings\":{\"requestsToTrack\":3,\"statusCode\":200},\"createdAt\":[TIMESTAMP]},{\"id\":\"[UUID]\",\"name\":\"name_two\",\"path\":\"/path\",\"method\":\"GET\",\"settings\":{\"requestsToTrack\":3,\"statusCode\":200},\"createdAt\":[TIMESTAMP]}]""###
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn properly_handles_responders_create_operation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebhooksResponders,
            Some(UtilsActionParams::json(json!({
                "name": "name_one",
                "path": "/",
                "method": "GET",
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
        let webhooks = api.webhooks();
        let responder = webhooks.get_responders(mock_user.id).await?.pop().unwrap();
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&responder.id.to_string(), "[UUID]");
        settings.add_filter(
            &responder.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );

        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""{\"id\":\"[UUID]\",\"name\":\"name_one\",\"path\":\"/\",\"method\":\"GET\",\"settings\":{\"requestsToTrack\":3,\"statusCode\":200},\"createdAt\":[TIMESTAMP]}""###
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn properly_handles_responders_update_operation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Get,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        script: None,
                        status_code: 200,
                        body: None,
                        headers: None,
                    },
                },
            )
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
                "path": "/",
                "method": "GET",
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
        let updated_responder = webhooks
            .get_responder(mock_user.id, responder.id)
            .await?
            .unwrap();
        assert_eq!(
            updated_responder,
            Responder {
                id: responder.id,
                name: "name_one_updated".to_string(),
                path: "/".to_string(),
                method: ResponderMethod::Get,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    script: None,
                    status_code: 200,
                    body: None,
                    headers: None,
                },
                created_at: responder.created_at
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_handles_responder_delete_operation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Get,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        script: None,
                        status_code: 200,
                        body: None,
                        headers: None,
                    },
                },
            )
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
        let deleted_responder = webhooks.get_responder(mock_user.id, responder.id).await?;
        assert!(deleted_responder.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn properly_handles_get_history_operation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert responders and requests.
        let webhooks = api.webhooks();
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Get,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        script: None,
                        status_code: 200,
                        body: None,
                        headers: None,
                    },
                },
            )
            .await?;
        let request_one = webhooks
            .create_responder_request(
                mock_user.id,
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
                mock_user.id,
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
                resource_id: responder.id,
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

    #[tokio::test]
    async fn properly_handles_clear_history_operation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert responder and requests.
        let webhooks = api.webhooks();
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Get,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        script: None,
                        status_code: 200,
                        body: None,
                        headers: None,
                    },
                },
            )
            .await?;
        webhooks
            .create_responder_request(
                mock_user.id,
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
                mock_user.id,
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
            webhooks
                .get_responder_requests(mock_user.id, responder.id)
                .await?
                .len(),
            2
        );

        let action_result = webhooks_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: responder.id,
                operation: UtilsResourceOperation::WebhooksRespondersClearHistory,
            },
            UtilsResource::WebhooksResponders,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        assert!(webhooks
            .get_responder_requests(mock_user.id, responder.id)
            .await?
            .is_empty());

        Ok(())
    }
}
