mod responders_create_params;
mod responders_request_create_params;
mod responders_update_params;

pub use self::{
    responders_create_params::RespondersCreateParams,
    responders_request_create_params::RespondersRequestCreateParams,
    responders_update_params::RespondersUpdateParams,
};
use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    security::USER_HANDLE_LENGTH_BYTES,
    users::User,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        webhooks::{
            Responder, ResponderMethod, ResponderPathType, ResponderRequest, ResponderStats,
        },
    },
};
use anyhow::bail;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

pub struct WebhooksApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> WebhooksApiExt<'a, 'u, DR, ET> {
    /// Creates Webhooks API.
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Retrieves all responders that belong to the specified user.
    pub async fn get_responders(&self) -> anyhow::Result<Vec<Responder>> {
        self.api.db.webhooks().get_responders(self.user.id).await
    }

    /// Retrieves stats for all responders that belong to the specified user.
    pub async fn get_responders_stats(&self) -> anyhow::Result<Vec<ResponderStats>> {
        self.api
            .db
            .webhooks()
            .get_responders_stats(self.user.id)
            .await
    }

    /// Returns responder by its ID.
    pub async fn get_responder(&self, id: Uuid) -> anyhow::Result<Option<Responder>> {
        self.api.db.webhooks().get_responder(self.user.id, id).await
    }

    /// Returns responder for specified subdomain prefix, path and method, if any.
    pub async fn find_responder(
        &self,
        subdomain_prefix: Option<&str>,
        path: &str,
        method: ResponderMethod,
    ) -> anyhow::Result<Option<Responder>> {
        if subdomain_prefix.is_some() {
            let features = self.user.subscription.get_features(&self.api.config);
            if !features.config.webhooks.responder_custom_subdomain_prefix {
                bail!(SecutilsError::client(
                    "Responder subdomain prefixes are not allowed."
                ));
            }
        }

        self.api
            .db
            .webhooks()
            .find_responder(self.user.id, subdomain_prefix, path, method)
            .await
    }

    /// Creates responder with the specified parameters and stores it in the database.
    pub async fn create_responder(
        &self,
        params: RespondersCreateParams,
    ) -> anyhow::Result<Responder> {
        // Preserve timestamp only up to seconds.
        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        let responder = Responder {
            id: Uuid::now_v7(),
            name: params.name,
            location: params.location,
            method: params.method,
            enabled: params.enabled,
            settings: params.settings,
            created_at,
            updated_at: created_at,
        };

        self.validate_responder(&responder)?;

        self.api
            .db
            .webhooks()
            .insert_responder(self.user.id, &responder)
            .await?;

        Ok(responder)
    }

    /// Updates responder.
    pub async fn update_responder(
        &self,
        id: Uuid,
        params: RespondersUpdateParams,
    ) -> anyhow::Result<Responder> {
        if params.name.is_none()
            && params.location.is_none()
            && params.method.is_none()
            && params.enabled.is_none()
            && params.settings.is_none()
        {
            bail!(SecutilsError::client(format!(
                "Either new name, path, method, enabled or settings should be provided ({id})."
            )));
        }

        let Some(existing_responder) = self.get_responder(id).await? else {
            bail!(SecutilsError::client(format!(
                "Responder ('{id}') is not found."
            )));
        };

        let responder = Responder {
            name: params.name.unwrap_or(existing_responder.name),
            location: params.location.unwrap_or(existing_responder.location),
            method: params.method.unwrap_or(existing_responder.method),
            enabled: params.enabled.unwrap_or(existing_responder.enabled),
            settings: params.settings.unwrap_or(existing_responder.settings),
            // Preserve timestamp only up to seconds.
            updated_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            ..existing_responder
        };

        self.validate_responder(&responder)?;

        self.api
            .db
            .webhooks()
            .update_responder(self.user.id, &responder)
            .await?;

        Ok(responder)
    }

    /// Removes responder by its ID.
    pub async fn remove_responder(&self, id: Uuid) -> anyhow::Result<()> {
        self.api
            .db
            .webhooks()
            .remove_responder(self.user.id, id)
            .await
    }

    // Persists request for the specified responder.
    pub async fn create_responder_request<'r>(
        &self,
        responder_id: Uuid,
        params: RespondersRequestCreateParams<'r>,
    ) -> anyhow::Result<Option<ResponderRequest<'r>>> {
        let Some(responder) = self.get_responder(responder_id).await? else {
            bail!(SecutilsError::client(format!(
                "Responder ('{responder_id}') is not found."
            )));
        };

        let features = self.user.subscription.get_features(&self.api.config);
        let max_requests = std::cmp::min(
            responder.settings.requests_to_track,
            features.config.webhooks.responder_requests,
        );
        if max_requests == 0 {
            return Ok(None);
        }

        let webhooks = self.api.db.webhooks();
        let requests = webhooks
            .get_responder_requests(self.user.id, responder.id)
            .await?;

        let request = ResponderRequest {
            id: Uuid::now_v7(),
            responder_id,
            client_address: params.client_address,
            method: params.method,
            headers: params.headers,
            url: params.url,
            body: params.body,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        Self::validate_responder_request(&responder, &request)?;

        // Insert new revision.
        webhooks
            .insert_responder_request(self.user.id, &request)
            .await?;

        // Enforce requests limit and displace old ones.
        if requests.len() >= max_requests {
            let requests_to_remove = requests.len() - max_requests + 1;
            for request_to_remove in requests.iter().take(requests_to_remove) {
                webhooks
                    .remove_responder_request(self.user.id, responder.id, request_to_remove.id)
                    .await?;
            }
        }

        Ok(Some(request))
    }

    /// Returns all stored webpage resources tracker history.
    pub async fn get_responder_requests(
        &self,
        responder_id: Uuid,
    ) -> anyhow::Result<Vec<ResponderRequest<'static>>> {
        if self.get_responder(responder_id).await?.is_none() {
            bail!(SecutilsError::client(format!(
                "Responder ('{responder_id}') is not found."
            )));
        };

        self.api
            .db
            .webhooks()
            .get_responder_requests(self.user.id, responder_id)
            .await
    }

    /// Removes all persisted requests for the specified responder.
    pub async fn clear_responder_requests(&self, responder_id: Uuid) -> anyhow::Result<()> {
        self.api
            .db
            .webhooks()
            .clear_responder_requests(self.user.id, responder_id)
            .await
    }

    fn validate_responder(&self, responder: &Responder) -> anyhow::Result<()> {
        if responder.name.is_empty() {
            bail!(SecutilsError::client("Responder name cannot be empty.",));
        }

        if responder.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Responder name cannot be longer than {MAX_UTILS_ENTITY_NAME_LENGTH} characters."
            )));
        }

        if responder.location.path.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Responder location path cannot be longer than {MAX_UTILS_ENTITY_NAME_LENGTH} characters."
            )));
        }

        let is_path_valid = responder.location.path.starts_with('/')
            && (responder.location.path.len() == 1 || !responder.location.path.ends_with('/'));
        if !is_path_valid {
            bail!(SecutilsError::client(
                "Responder location paths must begin with '/' and should not end with '/'."
            ));
        }

        let features = self.user.subscription.get_features(&self.api.config);
        if let Some(ref subdomain_prefix) = responder.location.subdomain_prefix {
            if !features.config.webhooks.responder_custom_subdomain_prefix {
                bail!(SecutilsError::client(
                    "Responder subdomain prefixes are not allowed."
                ));
            }

            let Some(public_host) = self.api.config.public_url.host_str() else {
                bail!(SecutilsError::client(
                    "Public URL doesn't have a host, cannot validate responder subdomain prefix."
                ));
            };

            if !self.is_valid_webhooks_subdomain_prefix(public_host, subdomain_prefix) {
                bail!(SecutilsError::client(format!(
                    "Responder subdomain prefix ('{subdomain_prefix}') is not valid."
                )));
            }
        }

        if !(100..=999).contains(&responder.settings.status_code) {
            bail!(SecutilsError::client(format!(
                "Responder status code should have a value between 100 and 999, but received {}.",
                responder.settings.status_code
            )));
        }

        if !(0..=features.config.webhooks.responder_requests)
            .contains(&responder.settings.requests_to_track)
        {
            bail!(SecutilsError::client(format!(
                "Responder can track only up to {} requests, but received {}.",
                features.config.webhooks.responder_requests, responder.settings.requests_to_track
            )));
        }

        if let Some(ref script) = responder.settings.script
            && script.is_empty()
        {
            bail!(SecutilsError::client("Responder script cannot be empty."));
        }

        Ok(())
    }

    fn validate_responder_request(
        responder: &Responder,
        request: &ResponderRequest,
    ) -> anyhow::Result<()> {
        let request_url =
            Url::parse(&format!("https://localhost{}", request.url)).map_err(|_| {
                SecutilsError::client(format!(
                    "Responder request URL ('{}') is not valid.",
                    request.url
                ))
            })?;

        let valid_request = match responder.location.path_type {
            ResponderPathType::Exact => responder.location.path == request_url.path(),
            ResponderPathType::Prefix => request_url.path().starts_with(&responder.location.path),
        };
        if !valid_request {
            bail!(SecutilsError::client(format!(
                "Responder request path ('{}') does not match responder path ('{:?}').",
                request_url.path(),
                responder.location
            )));
        }

        Ok(())
    }

    fn is_valid_webhooks_subdomain_prefix(
        &self,
        public_host: &str,
        subdomain_prefix: &str,
    ) -> bool {
        // Subdomain prefix should not contain dots to not add nested DNS labels.
        if subdomain_prefix.contains('.') {
            return false;
        }

        let webhooks_host = format!(
            "{subdomain_prefix}-{}.webhooks.{public_host}",
            // Add a bit of padding in case public_hostname changes length significantly in the
            // future making subdomain length invalid.
            "a".repeat(USER_HANDLE_LENGTH_BYTES + 10),
        );

        // First, check if it's a valid subdomain in general.
        if addr::parse_domain_name(&webhooks_host).is_err() {
            return false;
        };

        // Then, use URL parser to make sure subdomain is valid as is and doesn't require any
        // transformations (e.g., puny code conversion).
        let Ok(webhooks_url) = Url::parse(&format!("https://{webhooks_host}")) else {
            return false;
        };

        let webhooks_url_host = webhooks_url.host_str();
        webhooks_url_host == Some(&webhooks_host)
    }
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with responders.
    pub fn webhooks(&'a self, user: &'u User) -> WebhooksApiExt<'a, 'u, DR, ET> {
        WebhooksApiExt::new(self, user)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        tests::{mock_api, mock_user},
        utils::webhooks::{
            Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
            ResponderStats, RespondersRequestCreateParams,
            api_ext::{RespondersCreateParams, RespondersUpdateParams},
        },
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::{borrow::Cow, slice};
    use uuid::uuid;

    fn get_request_create_params(url: &str) -> RespondersRequestCreateParams<'_> {
        RespondersRequestCreateParams {
            client_address: None,
            method: Cow::Borrowed("POST"),
            headers: None,
            url: Cow::Borrowed(url),
            body: None,
        }
    }

    #[sqlx::test]
    async fn properly_creates_new_responder(pool: PgPool) -> anyhow::Result<()> {
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
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 302,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: Some("return { body: `custom body` };".to_string()),
                },
            })
            .await?;

        assert_eq!(
            responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_responder_at_creation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            script: Some("return { body: `custom body` };".to_string()),
        };

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "a".repeat(101),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder name cannot be longer than 100 characters.""###
        );

        // Empty path.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder location paths must begin with '/' and should not end with '/'.""###
        );

        // Very long path.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/a".repeat(51),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder location path cannot be longer than 100 characters.""###
        );

        // Invalid path start
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "path".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder location paths must begin with '/' and should not end with '/'.""###
        );

        // Invalid path end
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path/".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder location paths must begin with '/' and should not end with '/'.""###
        );

        // Empty subdomain prefix.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("".to_string())
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder subdomain prefix ('') is not valid.""###
        );

        // Subdomain prefix with dots.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("sub.sub".to_string())
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder subdomain prefix ('sub.sub') is not valid.""###
        );

        // Invalid subdomain prefix.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("сабдомейн".to_string())
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder subdomain prefix ('сабдомейн') is not valid.""###
        );

        // Long subdomain prefix.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("s".repeat(201))
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: settings.clone()
            }).await),
            @r###""Responder subdomain prefix ('sssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssss') is not valid.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    status_code: 99,
                    ..settings.clone()
                }
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 99.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    status_code: 1000,
                    ..settings.clone()
                }
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 1000.""###
        );

        // Too many requests to track.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                   requests_to_track: 101,
                    ..settings.clone()
                }
            }).await),
            @r###""Responder can track only up to 30 requests, but received 101.""###
        );

        // Invalid script.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder(RespondersCreateParams {
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                   script: Some("".to_string()),
                    ..settings.clone()
                }
            }).await),
            @r###""Responder script cannot be empty.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_updates_responder(pool: PgPool) -> anyhow::Result<()> {
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
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                },
            })
            .await?;

        // Update enabled.
        let updated_responder = webhooks
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    location: None,
                    method: None,
                    enabled: Some(false),
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            enabled: false,
            updated_at: updated_responder.updated_at,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        // Update name.
        let updated_responder = webhooks
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    name: Some("name_two".to_string()),
                    location: None,
                    method: None,
                    enabled: None,
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            enabled: false,
            updated_at: updated_responder.updated_at,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        // Update path.
        let updated_responder = webhooks
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    location: Some(ResponderLocation {
                        path_type: ResponderPathType::Exact,
                        path: "/path".to_string(),
                        subdomain_prefix: None,
                    }),
                    method: None,
                    enabled: None,
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/path".to_string(),
                subdomain_prefix: None,
            },
            enabled: false,
            updated_at: updated_responder.updated_at,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        // Update subdomain prefix.
        let updated_responder = webhooks
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    location: Some(ResponderLocation {
                        path_type: ResponderPathType::Prefix,
                        path: "/path".to_string(),
                        subdomain_prefix: Some("sub".to_string()),
                    }),
                    method: None,
                    enabled: None,
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/path".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            },
            enabled: false,
            updated_at: updated_responder.updated_at,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        // Update method.
        let updated_responder = webhooks
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    location: None,
                    method: Some(ResponderMethod::Post),
                    enabled: None,
                    settings: None,
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/path".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            },
            method: ResponderMethod::Post,
            enabled: false,
            updated_at: updated_responder.updated_at,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        // Update setting.
        let updated_responder = webhooks
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    name: None,
                    location: None,
                    method: None,
                    enabled: None,
                    settings: Some(ResponderSettings {
                        requests_to_track: 13,
                        status_code: 789,
                        body: Some("some-new-body".to_string()),
                        headers: Some(vec![("new-key".to_string(), "value".to_string())]),
                        script: Some("return { body: `custom body` };".to_string()),
                    }),
                },
            )
            .await?;
        let expected_responder = Responder {
            name: "name_two".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/path".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            },
            method: ResponderMethod::Post,
            enabled: false,
            settings: ResponderSettings {
                requests_to_track: 13,
                status_code: 789,
                body: Some("some-new-body".to_string()),
                headers: Some(vec![("new-key".to_string(), "value".to_string())]),
                script: Some("return { body: `custom body` };".to_string()),
            },
            updated_at: updated_responder.updated_at,
            ..responder.clone()
        };
        assert_eq!(expected_responder, updated_responder);
        assert_eq!(
            expected_responder,
            webhooks.get_responder(responder.id).await?.unwrap()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_responder_at_update(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty parameters.
        let update_result = update_and_fail(
            webhooks
                .update_responder(
                    responder.id,
                    RespondersUpdateParams {
                        name: None,
                        location: None,
                        method: None,
                        enabled: None,
                        settings: None,
                    },
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!(
                "Either new name, path, method, enabled or settings should be provided ({}).",
                responder.id
            )
        );

        // Non-existent responder.
        let update_result = update_and_fail(
            webhooks
                .update_responder(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    RespondersUpdateParams {
                        name: Some("some-new-name".to_string()),
                        location: None,
                        method: None,
                        enabled: None,
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
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: Some("".to_string()),
                location: None,
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: Some("a".repeat(101)),
                location: None,
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder name cannot be longer than 100 characters.""###
        );

        // Empty path.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "".to_string(),
                    subdomain_prefix: None
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder location paths must begin with '/' and should not end with '/'.""###
        );

        // Very long path.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/a".repeat(51),
                    subdomain_prefix: None
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder location path cannot be longer than 100 characters.""###
        );

        // Invalid path start
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "path".to_string(),
                    subdomain_prefix: None
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder location paths must begin with '/' and should not end with '/'.""###
        );

        // Invalid path end
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path/".to_string(),
                    subdomain_prefix: None
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder location paths must begin with '/' and should not end with '/'.""###
        );

        // Empty subdomain prefix.
        assert_debug_snapshot!(
             update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("".to_string())
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder subdomain prefix ('') is not valid.""###
        );

        // Subdomain prefix with dots.
        assert_debug_snapshot!(
             update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("sub.sub".to_string())
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder subdomain prefix ('sub.sub') is not valid.""###
        );

        // Invalid subdomain prefix.
        assert_debug_snapshot!(
             update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("сабдомейн".to_string())
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder subdomain prefix ('сабдомейн') is not valid.""###
        );

        // Long subdomain prefix.
        assert_debug_snapshot!(
             update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("s".repeat(201))
                }),
                method: None,
                enabled: None,
                settings: None
            }).await),
            @r###""Responder subdomain prefix ('sssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssss') is not valid.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: Some(ResponderSettings {
                    status_code: 99,
                    ..settings.clone()
                })
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 99.""###
        );

        // Invalid status code
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: Some(ResponderSettings {
                    status_code: 1000,
                    ..settings.clone()
                })
            }).await),
            @r###""Responder status code should have a value between 100 and 999, but received 1000.""###
        );

        // Too many requests to track.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 101,
                    ..settings.clone()
                })
            }).await),
            @r###""Responder can track only up to 30 requests, but received 101.""###
        );

        // Invalid script.
        assert_debug_snapshot!(
            update_and_fail(webhooks.update_responder(responder.id, RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: Some(ResponderSettings {
                    script: Some("".to_string()),
                    ..settings.clone()
                })
            }).await),
            @r###""Responder script cannot be empty.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_find_responders(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };

        let responders = [
            webhooks
                .create_responder(RespondersCreateParams {
                    name: "name_one".to_string(),
                    location: ResponderLocation {
                        path_type: ResponderPathType::Exact,
                        path: "/".to_string(),
                        subdomain_prefix: None,
                    },
                    method: ResponderMethod::Any,
                    enabled: true,
                    settings: settings.clone(),
                })
                .await?,
            webhooks
                .create_responder(RespondersCreateParams {
                    name: "name_two".to_string(),
                    location: ResponderLocation {
                        path_type: ResponderPathType::Prefix,
                        path: "/path".to_string(),
                        subdomain_prefix: Some("sub".to_string()),
                    },
                    method: ResponderMethod::Post,
                    enabled: true,
                    settings: settings.clone(),
                })
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
                webhooks.find_responder(None, "/", method).await?,
                Some(responders[0].clone())
            );

            if matches!(method, ResponderMethod::Post) {
                assert_eq!(
                    webhooks
                        .find_responder(Some("sub"), "/path", method)
                        .await?,
                    Some(responders[1].clone())
                );
            } else {
                assert_eq!(
                    webhooks
                        .find_responder(Some("sub"), "/path", method)
                        .await?,
                    None
                );
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn properly_removes_responders(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
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
                enabled: true,
                settings: settings.clone(),
            })
            .await?;

        assert_eq!(
            webhooks.get_responders().await?,
            [responder_one.clone(), responder_two.clone()]
        );

        webhooks.remove_responder(responder_one.id).await?;
        assert_eq!(
            webhooks.get_responders().await?,
            slice::from_ref(&responder_two)
        );

        webhooks.remove_responder(responder_two.id).await?;
        assert!(webhooks.get_responders().await?.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn properly_returns_all_responders(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        assert!(webhooks.get_responders().await?.is_empty());

        let settings = ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;
        assert_eq!(
            webhooks.get_responders().await?,
            vec![responder_one.clone()],
        );
        let responder_two = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_two".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: false,
                settings: settings.clone(),
            })
            .await?;

        assert_eq!(
            webhooks.get_responders().await?,
            vec![responder_one.clone(), responder_two.clone()],
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_returns_all_responders_stats(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        assert!(webhooks.get_responders().await?.is_empty());

        let settings = ResponderSettings {
            requests_to_track: 10,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
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
                method: ResponderMethod::Any,
                enabled: false,
                settings: settings.clone(),
            })
            .await?;

        assert!(webhooks.get_responders_stats().await?.is_empty());

        let request_one = webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/?query=value"))
            .await?
            .unwrap();
        assert_eq!(
            webhooks.get_responders_stats().await?,
            vec![ResponderStats {
                responder_id: responder_one.id,
                request_count: 1,
                last_requested_at: Some(request_one.created_at),
            }]
        );

        let request_two = webhooks
            .create_responder_request(
                responder_two.id,
                get_request_create_params("/path?query=value"),
            )
            .await?
            .unwrap();
        assert_eq!(
            webhooks.get_responders_stats().await?,
            vec![
                ResponderStats {
                    responder_id: responder_one.id,
                    request_count: 1,
                    last_requested_at: Some(request_one.created_at),
                },
                ResponderStats {
                    responder_id: responder_two.id,
                    request_count: 1,
                    last_requested_at: Some(request_two.created_at),
                }
            ]
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_creates_responder_requests(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;
        let responder_two = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_two".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/two".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: false,
                settings: settings.clone(),
            })
            .await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert!(responder_one_requests.is_empty());
        assert!(responder_two_requests.is_empty());

        webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/?query=value"))
            .await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert_eq!(responder_one_requests.len(), 1);
        assert_eq!(responder_one_requests[0].responder_id, responder_one.id);
        assert_eq!(responder_one_requests[0].method, Cow::Borrowed("POST"));
        assert!(responder_two_requests.is_empty());

        webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/"))
            .await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert!(responder_two_requests.is_empty());

        webhooks
            .create_responder_request(
                responder_two.id,
                get_request_create_params("/two?query=value"),
            )
            .await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert_eq!(responder_two_requests.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_responder_request_at_creation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Paths do not match.
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder_request(
                responder.id,
                get_request_create_params("/"),
            ).await),
            @r###""Responder request path ('/') does not match responder path ('/path (Exact)').""###
        );
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder_request(
                responder.id,
                get_request_create_params("/?query=value"),
            ).await),
            @r###""Responder request path ('/') does not match responder path ('/path (Exact)').""###
        );
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder_request(
                responder.id,
                get_request_create_params("/other-path"),
            ).await),
            @r###""Responder request path ('/other-path') does not match responder path ('/path (Exact)').""###
        );
        assert_debug_snapshot!(
            create_and_fail(webhooks.create_responder_request(
                responder.id,
                get_request_create_params("/other-path?query=value"),
            ).await),
            @r###""Responder request path ('/other-path') does not match responder path ('/path (Exact)').""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_removes_requests_when_responder_is_removed(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;
        let responder_two = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_two".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/two".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;

        webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/?query=value"))
            .await?;
        webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/"))
            .await?;
        webhooks
            .create_responder_request(
                responder_two.id,
                get_request_create_params("/two?query=value"),
            )
            .await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert_eq!(responder_two_requests.len(), 1);

        webhooks.remove_responder(responder_one.id).await?;

        assert!(
            webhooks
                .get_responder_requests(responder_one.id)
                .await
                .is_err()
        );
        assert!(
            api.db
                .webhooks()
                .get_responder_requests(mock_user.id, responder_one.id)
                .await?
                .is_empty()
        );

        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert_eq!(responder_two_requests.len(), 1);

        webhooks.remove_responder(responder_two.id).await?;

        assert!(
            webhooks
                .get_responder_requests(responder_two.id)
                .await
                .is_err()
        );
        assert!(
            api.db
                .webhooks()
                .get_responder_requests(mock_user.id, responder_two.id)
                .await?
                .is_empty()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_clears_requests(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let webhooks = api.webhooks(&mock_user);
        let settings = ResponderSettings {
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
        };
        let responder_one = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: settings.clone(),
            })
            .await?;
        let responder_two = webhooks
            .create_responder(RespondersCreateParams {
                name: "name_two".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/two".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: false,
                settings: settings.clone(),
            })
            .await?;

        webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/?query=value"))
            .await?;
        webhooks
            .create_responder_request(responder_one.id, get_request_create_params("/"))
            .await?;
        webhooks
            .create_responder_request(
                responder_two.id,
                get_request_create_params("/two?query=value"),
            )
            .await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert_eq!(responder_one_requests.len(), 2);
        assert_eq!(responder_two_requests.len(), 1);

        webhooks.clear_responder_requests(responder_one.id).await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert!(responder_one_requests.is_empty());
        assert_eq!(responder_two_requests.len(), 1);

        webhooks.clear_responder_requests(responder_two.id).await?;

        let responder_one_requests = webhooks.get_responder_requests(responder_one.id).await?;
        let responder_two_requests = webhooks.get_responder_requests(responder_two.id).await?;
        assert!(responder_one_requests.is_empty());
        assert!(responder_two_requests.is_empty());

        Ok(())
    }
}
