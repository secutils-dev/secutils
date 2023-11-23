mod responders_create_params;
mod responders_request_create_params;
mod responders_update_params;

use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::UserId,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, webhooks::ResponderRequest,
        Responder, ResponderMethod,
    },
};
use anyhow::bail;
use time::OffsetDateTime;
use uuid::Uuid;

pub use self::{
    responders_create_params::RespondersCreateParams,
    responders_request_create_params::RespondersRequestCreateParams,
    responders_update_params::RespondersUpdateParams,
};

pub struct WebhooksApiExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> WebhooksApiExt<'a, DR, ET> {
    /// Creates Webhooks API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Retrieves all responders that belong to the specified user.
    pub async fn get_responders(&self, user_id: UserId) -> anyhow::Result<Vec<Responder>> {
        self.api.db.webhooks().get_responders(user_id).await
    }

    /// Returns responder by its ID.
    pub async fn get_responder(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<Responder>> {
        self.api.db.webhooks().get_responder(user_id, id).await
    }

    /// Returns responder for specified path and method, if any.
    pub async fn find_responder(
        &self,
        user_id: UserId,
        path: &str,
        method: ResponderMethod,
    ) -> anyhow::Result<Option<Responder>> {
        self.api
            .db
            .webhooks()
            .find_responder(user_id, path, method)
            .await
    }

    /// Creates responder with the specified parameters and stores it in the database.
    pub async fn create_responder(
        &self,
        user_id: UserId,
        params: RespondersCreateParams,
    ) -> anyhow::Result<Responder> {
        let responder = Responder {
            id: Uuid::now_v7(),
            name: params.name,
            path: params.path,
            method: params.method,
            settings: params.settings,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        Self::validate_responder(&responder)?;

        self.api
            .db
            .webhooks()
            .insert_responder(user_id, &responder)
            .await?;

        Ok(responder)
    }

    /// Updates responder.
    pub async fn update_responder(
        &self,
        user_id: UserId,
        id: Uuid,
        params: RespondersUpdateParams,
    ) -> anyhow::Result<Responder> {
        if params.name.is_none()
            && params.path.is_none()
            && params.method.is_none()
            && params.settings.is_none()
        {
            bail!(SecutilsError::client(format!(
                "Either new name, path, method or settings should be provided ({id})."
            )));
        }

        let Some(existing_responder) = self.get_responder(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Responder ('{id}') is not found."
            )));
        };

        let responder = Responder {
            name: params.name.unwrap_or(existing_responder.name),
            path: params.path.unwrap_or(existing_responder.path),
            method: params.method.unwrap_or(existing_responder.method),
            settings: params.settings.unwrap_or(existing_responder.settings),
            ..existing_responder
        };

        Self::validate_responder(&responder)?;

        self.api
            .db
            .webhooks()
            .update_responder(user_id, &responder)
            .await?;

        Ok(responder)
    }

    /// Removes responder by its ID.
    pub async fn remove_responder(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        self.api.db.webhooks().remove_responder(user_id, id).await
    }

    // Persists request for the specified responder.
    pub async fn create_responder_request<'r>(
        &self,
        user_id: UserId,
        responder_id: Uuid,
        params: RespondersRequestCreateParams<'r>,
    ) -> anyhow::Result<Option<ResponderRequest<'r>>> {
        let Some(responder) = self.get_responder(user_id, responder_id).await? else {
            bail!(SecutilsError::client(format!(
                "Responder ('{responder_id}') is not found."
            )));
        };

        if responder.settings.requests_to_track == 0 {
            return Ok(None);
        }

        let webhooks = self.api.db.webhooks();
        let requests = webhooks
            .get_responder_requests(user_id, responder.id)
            .await?;

        let request = ResponderRequest {
            id: Uuid::now_v7(),
            responder_id,
            client_address: params.client_address,
            method: params.method,
            headers: params.headers,
            body: params.body,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        // Insert new revision.
        webhooks.insert_responder_request(user_id, &request).await?;

        // Enforce requests limit and displace old ones.
        if requests.len() >= responder.settings.requests_to_track {
            let requests_to_remove = requests.len() - responder.settings.requests_to_track + 1;
            for request_to_remove in requests.iter().take(requests_to_remove) {
                webhooks
                    .remove_responder_request(user_id, responder.id, request_to_remove.id)
                    .await?;
            }
        }

        Ok(Some(request))
    }

    /// Returns all stored webpage resources tracker history.
    pub async fn get_responder_requests(
        &self,
        user_id: UserId,
        responder_id: Uuid,
    ) -> anyhow::Result<Vec<ResponderRequest<'static>>> {
        if self.get_responder(user_id, responder_id).await?.is_none() {
            bail!(SecutilsError::client(format!(
                "Responder ('{responder_id}') is not found."
            )));
        };

        self.api
            .db
            .webhooks()
            .get_responder_requests(user_id, responder_id)
            .await
    }

    /// Removes all persisted requests for the specified responder.
    pub async fn clear_responder_requests(
        &self,
        user_id: UserId,
        responder_id: Uuid,
    ) -> anyhow::Result<()> {
        self.api
            .db
            .webhooks()
            .clear_responder_requests(user_id, responder_id)
            .await
    }

    fn validate_responder(responder: &Responder) -> anyhow::Result<()> {
        if responder.name.is_empty() {
            bail!(SecutilsError::client("Responder name cannot be empty.",));
        }

        if responder.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Responder name cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        if responder.path.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Responder path cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        let is_path_valid = responder.path.starts_with('/')
            && (responder.path.len() == 1 || !responder.path.ends_with('/'));
        if !is_path_valid {
            bail!(SecutilsError::client(
                "Responder paths must begin with '/' and should not end with '/'."
            ));
        }

        if !(100..=999).contains(&responder.settings.status_code) {
            bail!(SecutilsError::client(format!(
                "Responder status code should have a value between 100 and 999, but received {}.",
                responder.settings.status_code
            )));
        }

        if !(0..=100).contains(&responder.settings.requests_to_track) {
            bail!(SecutilsError::client(format!(
                "Responder can track only up to 100 requests, but received {}.",
                responder.settings.requests_to_track
            )));
        }

        Ok(())
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with responders.
    pub fn webhooks(&self) -> WebhooksApiExt<DR, ET> {
        WebhooksApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        tests::{mock_api, mock_user},
        utils::{
            Responder, ResponderMethod, ResponderSettings, RespondersCreateParams,
            RespondersRequestCreateParams, RespondersUpdateParams, WebhooksApiExt,
        },
    };
    use insta::assert_debug_snapshot;
    use std::{borrow::Cow, time::Duration};
    use uuid::uuid;

    fn get_request_create_params<'r>() -> RespondersRequestCreateParams<'r> {
        RespondersRequestCreateParams {
            client_address: None,
            method: Cow::Borrowed("POST"),
            headers: None,
            body: None,
        }
    }

    #[tokio::test]
    async fn properly_creates_new_responder() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = WebhooksApiExt::new(&api);
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        status_code: 302,
                        body: Some("body".to_string()),
                        headers: Some(vec![("key".to_string(), "value".to_string())]),
                        delay: Duration::from_millis(1000),
                    },
                },
            )
            .await?;

        assert_eq!(
            responder,
            webhooks
                .get_responder(mock_user.id, responder.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_responder_at_creation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = WebhooksApiExt::new(&api);
        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "".to_string(),
                path: "/".to_string(),
                method: ResponderMethod::Get,
                settings: settings.clone()
            }).await),
            @r###""Responder name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "a".repeat(101),
                path: "/".to_string(),
                method: ResponderMethod::Get,
                settings: settings.clone()
            }).await),
            @r###""Responder name cannot be longer than 100 characters.""###
        );

        // Empty path.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "".to_string(),
                method: ResponderMethod::Get,
                settings: settings.clone()
            }).await),
            @r###""Responder paths must begin with '/' and should not end with '/'.""###
        );

        // Very long path.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "/a".repeat(51),
                method: ResponderMethod::Get,
                settings: settings.clone()
            }).await),
            @r###""Responder path cannot be longer than 100 characters.""###
        );

        // Invalid path start
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "path".to_string(),
                method: ResponderMethod::Get,
                settings: settings.clone()
            }).await),
            @r###""Responder paths must begin with '/' and should not end with '/'.""###
        );

        // Invalid path end
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "/path/".to_string(),
                method: ResponderMethod::Get,
                settings: settings.clone()
            }).await),
            @r###""Responder paths must begin with '/' and should not end with '/'.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "/path".to_string(),
                method: ResponderMethod::Get,
                settings: ResponderSettings {
                    status_code: 99,
                    ..settings.clone()
                }
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 99.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "/path".to_string(),
                method: ResponderMethod::Get,
                settings: ResponderSettings {
                    status_code: 1000,
                    ..settings.clone()
                }
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 1000.""###
        );

        // Too many requests to track.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(mock_user.id, RespondersCreateParams {
                name: "some-name".to_string(),
                path: "/path".to_string(),
                method: ResponderMethod::Get,
                settings: ResponderSettings {
                   requests_to_track: 101,
                    ..settings.clone()
                }
            }).await),
            @r###""Responder can track only up to 100 requests, but received 101.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_content_security_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = WebhooksApiExt::new(&api);
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 0,
                        status_code: 200,
                        body: None,
                        headers: None,
                        delay: Duration::from_millis(1000),
                    },
                },
            )
            .await?;

        // Update name.
        let updated_responder = webhooks
            .update_responder(
                mock_user.id,
                responder.id,
                RespondersUpdateParams {
                    name: Some("name_two".to_string()),
                    path: None,
                    method: None,
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks
                .get_responder(mock_user.id, responder.id)
                .await?
                .unwrap()
        );

        // Update path.
        let updated_responder = webhooks
            .update_responder(
                mock_user.id,
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    path: Some("/path".to_string()),
                    method: None,
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            path: "/path".to_string(),
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks
                .get_responder(mock_user.id, responder.id)
                .await?
                .unwrap()
        );

        // Update method.
        let updated_responder = webhooks
            .update_responder(
                mock_user.id,
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    path: None,
                    method: Some(ResponderMethod::Post),
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            path: "/path".to_string(),
            method: ResponderMethod::Post,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks
                .get_responder(mock_user.id, responder.id)
                .await?
                .unwrap()
        );

        // Update setting.
        let updated_responder = webhooks
            .update_responder(
                mock_user.id,
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    path: None,
                    method: None,
                    settings: Some(ResponderSettings {
                        requests_to_track: 13,
                        status_code: 789,
                        body: Some("some-new-body".to_string()),
                        headers: Some(vec![("new-key".to_string(), "value".to_string())]),
                        delay: Duration::from_millis(2000),
                    }),
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            path: "/path".to_string(),
            method: ResponderMethod::Post,
            settings: ResponderSettings {
                requests_to_track: 13,
                status_code: 789,
                body: Some("some-new-body".to_string()),
                headers: Some(vec![("new-key".to_string(), "value".to_string())]),
                delay: Duration::from_millis(2000),
            },
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks
                .get_responder(mock_user.id, responder.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_responder_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };
        let responder = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty parameters.
        let update_result = update_and_fail(
            webhooks
                .update_responder(
                    mock_user.id,
                    responder.id,
                    RespondersUpdateParams {
                        name: None,
                        path: None,
                        method: None,
                        settings: None,
                    },
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!(
                "Either new name, path, method or settings should be provided ({}).",
                responder.id
            )
        );

        // Non-existent responder.
        let update_result = update_and_fail(
            webhooks
                .update_responder(
                    mock_user.id,
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    RespondersUpdateParams {
                        name: Some("some-new-name".to_string()),
                        path: None,
                        method: None,
                        settings: None,
                    },
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            "Responder ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: Some("".to_string()),
                path: None,
                method: None,
                settings: None
            }).await),
            @r###""Responder name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: Some("a".repeat(101)),
                path: None,
                method: None,
                settings: None
            }).await),
            @r###""Responder name cannot be longer than 100 characters.""###
        );

        // Empty path.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: Some("".to_string()),
                method: None,
                settings: None
            }).await),
            @r###""Responder paths must begin with '/' and should not end with '/'.""###
        );

        // Very long path.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: Some("/a".repeat(51)),
                method: None,
                settings: None
            }).await),
            @r###""Responder path cannot be longer than 100 characters.""###
        );

        // Invalid path start
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: Some("path".to_string()),
                method: None,
                settings: None
            }).await),
            @r###""Responder paths must begin with '/' and should not end with '/'.""###
        );

        // Invalid path end
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: Some("/path/".to_string()),
                method: None,
                settings: None
            }).await),
            @r###""Responder paths must begin with '/' and should not end with '/'.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: None,
                method: None,
                settings: Some(ResponderSettings {
                    status_code: 99,
                    ..settings.clone()
                })
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 99.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: None,
                method: None,
                settings: Some(ResponderSettings {
                    status_code: 1000,
                    ..settings.clone()
                })
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 1000.""###
        );

        // Too many requests to track.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(mock_user.id, responder.id, RespondersUpdateParams {
                name: None,
                path: None,
                method: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 101,
                    ..settings.clone()
                })
            }).await),
            @r###""Responder can track only up to 100 requests, but received 101.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_find_responders() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };

        let responders = [
            webhooks
                .create_responder(
                    mock_user.id,
                    RespondersCreateParams {
                        name: "name_one".to_string(),
                        path: "/".to_string(),
                        method: ResponderMethod::Any,
                        settings: settings.clone(),
                    },
                )
                .await?,
            webhooks
                .create_responder(
                    mock_user.id,
                    RespondersCreateParams {
                        name: "name_two".to_string(),
                        path: "/path".to_string(),
                        method: ResponderMethod::Post,
                        settings: settings.clone(),
                    },
                )
                .await?,
        ];

        for method in [
            ResponderMethod::Post,
            ResponderMethod::Get,
            ResponderMethod::Any,
            ResponderMethod::Delete,
            ResponderMethod::Connect,
            ResponderMethod::Head,
            ResponderMethod::Options,
            ResponderMethod::Patch,
            ResponderMethod::Put,
            ResponderMethod::Trace,
        ] {
            assert_eq!(
                webhooks.find_responder(mock_user.id, "/", method).await?,
                Some(responders[0].clone())
            );

            if matches!(method, ResponderMethod::Post) {
                assert_eq!(
                    webhooks
                        .find_responder(mock_user.id, "/path", method)
                        .await?,
                    Some(responders[1].clone())
                );
            } else {
                assert_eq!(
                    webhooks
                        .find_responder(mock_user.id, "/path", method)
                        .await?,
                    None
                );
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_responders() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };
        let responder_one = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
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
                    settings: settings.clone(),
                },
            )
            .await?;

        assert_eq!(
            webhooks.get_responders(mock_user.id).await?,
            [responder_one.clone(), responder_two.clone()]
        );

        webhooks
            .remove_responder(mock_user.id, responder_one.id)
            .await?;
        assert_eq!(
            webhooks.get_responders(mock_user.id).await?,
            [responder_two.clone()]
        );

        webhooks
            .remove_responder(mock_user.id, responder_two.id)
            .await?;
        assert!(webhooks.get_responders(mock_user.id).await?.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_all_responders() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        assert!(webhooks.get_responders(mock_user.id).await?.is_empty());

        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };
        let responder_one = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;
        assert_eq!(
            webhooks.get_responders(mock_user.id).await?,
            vec![responder_one.clone()],
        );
        let responder_two = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_two".to_string(),
                    path: "/path".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;

        assert_eq!(
            webhooks.get_responders(mock_user.id).await?,
            vec![responder_one.clone(), responder_two.clone()],
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_creates_responder_requests() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };
        let responder_one = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;
        let responder_two = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_two".to_string(),
                    path: "/two".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert!(responder_one_requests.is_empty());
        assert!(responder_two_requests.is_empty());

        webhooks
            .create_responder_request(mock_user.id, responder_one.id, get_request_create_params())
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert_eq!(responder_one_requests.len(), 1);
        assert_eq!(responder_one_requests[0].responder_id, responder_one.id);
        assert_eq!(responder_one_requests[0].method, Cow::Borrowed("POST"));
        assert!(responder_two_requests.is_empty());

        webhooks
            .create_responder_request(mock_user.id, responder_one.id, get_request_create_params())
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert!(responder_two_requests.is_empty());

        webhooks
            .create_responder_request(mock_user.id, responder_two.id, get_request_create_params())
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert_eq!(responder_two_requests.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_requests_when_responder_is_removed() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };
        let responder_one = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;
        let responder_two = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_two".to_string(),
                    path: "/two".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;

        webhooks
            .create_responder_request(mock_user.id, responder_one.id, get_request_create_params())
            .await?;
        webhooks
            .create_responder_request(mock_user.id, responder_one.id, get_request_create_params())
            .await?;
        webhooks
            .create_responder_request(mock_user.id, responder_two.id, get_request_create_params())
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert_eq!(responder_two_requests.len(), 1);

        webhooks
            .remove_responder(mock_user.id, responder_one.id)
            .await?;

        assert!(webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await
            .is_err());
        assert!(api
            .db
            .webhooks()
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?
            .is_empty());

        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert_eq!(responder_two_requests.len(), 1);

        webhooks
            .remove_responder(mock_user.id, responder_two.id)
            .await?;

        assert!(webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await
            .is_err());
        assert!(api
            .db
            .webhooks()
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_clears_requests() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks();
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: Duration::from_millis(1000),
        };
        let responder_one = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;
        let responder_two = webhooks
            .create_responder(
                mock_user.id,
                RespondersCreateParams {
                    name: "name_two".to_string(),
                    path: "/two".to_string(),
                    method: ResponderMethod::Any,
                    settings: settings.clone(),
                },
            )
            .await?;

        webhooks
            .create_responder_request(mock_user.id, responder_one.id, get_request_create_params())
            .await?;
        webhooks
            .create_responder_request(mock_user.id, responder_one.id, get_request_create_params())
            .await?;
        webhooks
            .create_responder_request(mock_user.id, responder_two.id, get_request_create_params())
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert_eq!(responder_two_requests.len(), 1);

        webhooks
            .clear_responder_requests(mock_user.id, responder_one.id)
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert!(responder_one_requests.is_empty());
        assert_eq!(responder_two_requests.len(), 1);

        webhooks
            .clear_responder_requests(mock_user.id, responder_two.id)
            .await?;

        let responder_one_requests = webhooks
            .get_responder_requests(mock_user.id, responder_one.id)
            .await?;
        let responder_two_requests = webhooks
            .get_responder_requests(mock_user.id, responder_two.id)
            .await?;
        assert!(responder_one_requests.is_empty());
        assert!(responder_two_requests.is_empty());

        Ok(())
    }
}
