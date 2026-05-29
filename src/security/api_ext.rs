use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    security::{
        Operator,
        credentials::Credentials,
        jwt::Claims,
        kratos::{Identity, RecoveryLink, Session},
    },
    users::{
        User, UserDataCloneSummary, UserId, UserSignupError, UserSubscription, clone_user_data,
    },
};
use actix_web::cookie::Cookie;
use anyhow::{Context, anyhow, bail};
use hex::ToHex;
use jsonwebtoken::{DecodingKey, Validation, decode};
use reqwest::StatusCode;
use serde_json::json;
use tracing::{error, warn};
use uuid::Uuid;

pub const USER_HANDLE_LENGTH_BYTES: usize = 8;

/// Secutils.dev security controller.
pub struct SecurityApiExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> SecurityApiExt<'a, DR, ET>
where
    ET::Error: EmailTransportError,
{
    /// Instantiates security API extension.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Signs up a user with the specified email and credentials. If the user with such email is
    /// already registered, this method throws.
    /// NOTE: User isn't required to activate profile right away and can use application without
    /// activation. After signup, we'll send an email with the activation code, and will re-send it
    /// after 7 days, then after 14 days, and after 30 days we'll terminate the account with a large
    /// warning in the application. Users will be able to request another activation link from their
    /// profile page.
    pub async fn signup(&self, user: &User) -> anyhow::Result<()> {
        // Check if the user with specified email already exists.
        if let Some(user) = self
            .api
            .users()
            .get(user.id)
            .await
            .with_context(|| "Failed to check if user already exists.")?
        {
            error!(user.id = %user.id, "User is already registered.");
            return Err(UserSignupError::EmailAlreadyRegistered.into());
        }

        // Use insert instead of upsert here to prevent multiple signup requests from the same user.
        // Consumer of the API is supposed to perform validation before invoking this method.
        self.api
            .db
            .insert_user(&user)
            .await
            .with_context(|| "Cannot signup user, failed to insert a new user.")?;

        Ok(())
    }

    /// Authenticates a user with the specified credentials.
    pub async fn authenticate(&self, credentials: &Credentials) -> anyhow::Result<Option<User>> {
        let identity = match credentials {
            Credentials::Jwt(token) => {
                self.get_identity_by_email(&self.get_jwt_claims(token).await?.sub)
                    .await
            }
            Credentials::SessionCookie(cookie) => self.get_identity_by_cookie(cookie).await,
            Credentials::ApiKey(token) => self.get_identity_by_api_key(token).await,
        }?;

        let Some(identity) = identity else {
            error!("Couldn't retrieve user identity with provided credentials.");
            return Ok(None);
        };

        let Some(user) = self.api.users().get(UserId::from(identity.id)).await? else {
            error!(user.id = %identity.id, "User doesn't exist.");
            return Ok(None);
        };

        let operators = self.api.config.security.operators.as_ref();
        Ok(Some(User {
            created_at: identity.created_at,
            is_activated: identity.is_activated(),
            is_operator: operators.is_some_and(|operators| operators.contains(&user.email)),
            ..user
        }))
    }

    /// Terminates user's subscription, removes Kratos identity, and user information. If the user
    /// or Kratos identity is found, return the user ID.
    pub async fn terminate(&self, user_email: &str) -> anyhow::Result<Option<UserId>> {
        // Check if the identity for the user with specified email exists.
        let identity = self
            .get_identity_by_email(user_email)
            .await
            .with_context(|| "Failed to retrieve user identity.")?;
        let user_id = if let Some(identity) = identity {
            self.delete_identity(identity.id).await?;
            Some(UserId::from(identity.id))
        } else {
            warn!("User with email `{user_email}` doesn't exist.");
            None
        };

        // Remove user and their data from the database.
        Ok(self
            .api
            .db
            .remove_user_by_email(user_email)
            .await?
            .or(user_id))
    }

    /// Creates a brand-new Kratos identity for the specified email. The identity is created
    /// without any credentials (the operator is expected to mint a recovery link via
    /// [`Self::create_recovery_link`] and let the destination user set a password through it).
    /// When `verified` is true, the email address is marked as already verified, skipping the
    /// usual email round-trip - appropriate for operator-driven clone flows.
    pub async fn create_identity(&self, email: &str, verified: bool) -> anyhow::Result<Identity> {
        let body = json!({
            "schema_id": "user",
            "traits": { "email": email },
            "verifiable_addresses": [{
                "value": email,
                "verified": verified,
                "via": "email",
                "status": if verified { "completed" } else { "pending" }
            }]
        });

        let request = self
            .api
            .network
            .http_client
            .request(
                reqwest::Method::POST,
                format!(
                    "{}admin/identities",
                    self.api.config.components.kratos_admin_url.as_str()
                ),
            )
            .json(&body)
            .build()
            .map_err(|err| {
                error!("Cannot build Kratos create-identity request: {err:?}");
                anyhow!(err)
            })?;

        let response = self
            .api
            .network
            .http_client
            .execute(request)
            .await
            .map_err(|err| {
                error!("Cannot execute Kratos create-identity request: {err:?}");
                anyhow!(err)
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("Kratos create-identity request failed with status `{status}` and body: {body}");
            bail!("Kratos create-identity request failed with status `{status}`.");
        }

        Ok(response.json::<Identity>().await?)
    }

    /// Mints a single-use Kratos admin recovery link for the specified identity. The operator
    /// follows the returned URL to set a password and complete the login.
    ///
    /// Uses Kratos's `POST /admin/recovery/code` endpoint (`createRecoveryCodeForIdentity`),
    /// which is the supported admin-driven flow regardless of whether the deployment is
    /// configured with `recovery.use = "code"` or `"link"`. The response shape is identical
    /// in both modes (a recovery URL + expiry), under the `code` strategy the response
    /// additionally carries a `recovery_code` the user enters at the URL.
    pub async fn create_recovery_link(
        &self,
        identity_id: Uuid,
        expires_in: &str,
    ) -> anyhow::Result<RecoveryLink> {
        let body = json!({
            "identity_id": identity_id,
            "expires_in": expires_in,
        });

        let request = self
            .api
            .network
            .http_client
            .request(
                reqwest::Method::POST,
                format!(
                    "{}admin/recovery/code",
                    self.api.config.components.kratos_admin_url.as_str()
                ),
            )
            .json(&body)
            .build()
            .map_err(|err| {
                error!("Cannot build Kratos create-recovery-link request: {err:?}");
                anyhow!(err)
            })?;

        let response = self
            .api
            .network
            .http_client
            .execute(request)
            .await
            .map_err(|err| {
                error!("Cannot execute Kratos create-recovery-link request: {err:?}");
                anyhow!(err)
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!(
                "Kratos create-recovery-link request failed with status `{status}` and body: {body}"
            );
            bail!("Kratos create-recovery-link request failed with status `{status}`.");
        }

        Ok(response.json::<RecoveryLink>().await?)
    }

    /// Copies every entity owned by `source` (tags, scripts, secrets, responders, certificate
    /// templates, private keys, content security policies, page/API trackers, settings) into
    /// `destination`, regenerating IDs as needed. Reuses the standard export/import pipeline,
    /// including history when `include_history` is true, so the same code paths that the public
    /// `/api/user/data/_export` and `/api/user/data/_import` endpoints traverse are exercised here.
    /// Secrets are round-tripped through the export-encryption layer using an ephemeral passphrase
    /// generated only for the duration of this call, the passphrase never leaves the process.
    pub async fn clone_data(
        &self,
        source: &User,
        destination: &User,
        include_history: bool,
    ) -> anyhow::Result<UserDataCloneSummary> {
        clone_user_data(self.api, source, destination, include_history).await
    }

    /// Checks if the user or service account with specified credentials is an operator.
    /// API keys are never allowed to act as operators.
    pub async fn get_operator(
        &self,
        credentials: &Credentials,
    ) -> anyhow::Result<Option<Operator>> {
        let operator_id = match credentials {
            // API keys cannot act as operators.
            Credentials::ApiKey(_) => return Ok(None),
            // If the user is authenticated with a session cookie, user's email is used as an
            // operator identifier.
            Credentials::SessionCookie(cooke) => {
                self.get_identity_by_cookie(cooke)
                    .await?
                    .ok_or_else(|| anyhow!("Session cookie is invalid"))?
                    .traits
                    .email
            }
            // For JWT, we treat `sub` claim as an operator identifier.
            Credentials::Jwt(token) => self.get_jwt_claims(token).await?.sub,
        };

        let operators = self.api.config.security.operators.as_ref();
        if operators.is_some_and(|operators| operators.contains(&operator_id)) {
            Ok(Some(Operator::new(operator_id)))
        } else {
            Ok(None)
        }
    }

    /// Tries to retrieve user identity from Kratos using specified credentials.
    async fn get_identity_by_cookie(
        &self,
        cookie: &Cookie<'_>,
    ) -> anyhow::Result<Option<Identity>> {
        let request_builder = self
            .api
            .network
            .http_client
            .request(
                reqwest::Method::GET,
                format!(
                    "{}sessions/whoami",
                    self.api.config.components.kratos_url.as_str()
                ),
            )
            .header(
                "Cookie",
                format!("{}={}", cookie.name(), cookie.value()).as_bytes(),
            );
        let request = match request_builder.build() {
            Ok(client) => client,
            Err(err) => {
                error!("Cannot build Kratos request: {err:?}");
                return Err(anyhow!(err));
            }
        };

        let response = match self.api.network.http_client.execute(request).await {
            Ok(response) => response,
            Err(err) => {
                error!("Cannot execute Kratos request: {err:?}");
                return Err(anyhow!(err));
            }
        };

        let response_status = response.status();
        if !response_status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return match response_status {
                StatusCode::UNAUTHORIZED => {
                    error!("Kratos request couldn't be authenticated: {error_text}");
                    Ok(None)
                }
                _ => {
                    error!(
                        "Kratos request failed with the status code `{response_status}` and body: {error_text}"
                    );
                    Err(anyhow!(
                        "Kratos request failed with the status code `{response_status}`."
                    ))
                }
            };
        }

        Ok(response
            .json::<Session>()
            .await
            .map(|session| session.identity)?)
    }

    /// Tries to retrieve user identity from Kratos using the specified email.
    async fn get_identity_by_email(&self, email: &str) -> anyhow::Result<Option<Identity>> {
        let request_builder = self.api.network.http_client.request(
            reqwest::Method::GET,
            format!(
                "{}admin/identities?credentials_identifier={}",
                self.api.config.components.kratos_admin_url.as_str(),
                urlencoding::encode(email)
            ),
        );

        let request = match request_builder.build() {
            Ok(client) => client,
            Err(err) => {
                error!("Cannot build Kratos request: {err:?}");
                bail!(err);
            }
        };

        let response = match self.api.network.http_client.execute(request).await {
            Ok(response) => response,
            Err(err) => {
                error!("Cannot execute Kratos request: {err:?}");
                bail!(err);
            }
        };

        let response_status = response.status();
        if !response_status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return match response_status {
                StatusCode::UNAUTHORIZED => {
                    error!("Kratos request couldn't be authenticated: {error_text}");
                    Ok(None)
                }
                _ => {
                    error!(
                        "Kratos request failed with the status code `{response_status}` and body: {error_text}"
                    );
                    Err(anyhow!(
                        "Kratos request failed with the status code `{response_status}`."
                    ))
                }
            };
        }

        Ok(response
            .json::<Vec<Identity>>()
            .await
            .map(|identities| identities.into_iter().next())?)
    }

    /// Validates an API key token and retrieves the corresponding Kratos identity.
    async fn get_identity_by_api_key(&self, token: &str) -> anyhow::Result<Option<Identity>> {
        let api_keys_api = self.api.api_keys_system();
        let Some(api_key) = api_keys_api.validate_api_key_token(token).await? else {
            return Ok(None);
        };

        let identity = self.get_identity_by_id(*api_key.user_id).await?;

        // Best-effort last_used_at update.
        api_keys_api.touch_api_key_last_used(api_key.id).await;

        Ok(identity)
    }

    /// Retrieves a Kratos identity by its UUID.
    async fn get_identity_by_id(&self, id: Uuid) -> anyhow::Result<Option<Identity>> {
        let request_builder = self.api.network.http_client.request(
            reqwest::Method::GET,
            format!(
                "{}admin/identities/{id}",
                self.api.config.components.kratos_admin_url.as_str(),
            ),
        );

        let request = match request_builder.build() {
            Ok(client) => client,
            Err(err) => {
                error!("Cannot build Kratos request: {err:?}");
                bail!(err);
            }
        };

        let response = match self.api.network.http_client.execute(request).await {
            Ok(response) => response,
            Err(err) => {
                error!("Cannot execute Kratos request: {err:?}");
                bail!(err);
            }
        };

        let response_status = response.status();
        if !response_status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return match response_status {
                StatusCode::NOT_FOUND => Ok(None),
                _ => {
                    error!(
                        "Kratos request failed with the status code `{response_status}` and body: {error_text}"
                    );
                    Err(anyhow!(
                        "Kratos request failed with the status code `{response_status}`."
                    ))
                }
            };
        }

        Ok(Some(response.json::<Identity>().await?))
    }

    /// Deletes user identity from Kratos.
    async fn delete_identity(&self, id: Uuid) -> anyhow::Result<()> {
        let request_builder = self.api.network.http_client.request(
            reqwest::Method::DELETE,
            format!(
                "{}admin/identities/{id}",
                self.api.config.components.kratos_admin_url.as_str()
            ),
        );

        let request = match request_builder.build() {
            Ok(client) => client,
            Err(err) => {
                error!("Cannot build Kratos DELETE identity request: {err:?}");
                bail!(err);
            }
        };

        let response = match self.api.network.http_client.execute(request).await {
            Ok(response) => response,
            Err(err) => {
                error!("Cannot execute Kratos DELETE identity request: {err:?}");
                bail!(err);
            }
        };

        let response_status = response.status();
        if !response_status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Kratos DELETE identity request failed with the status code `{response_status}` and body: {error_text}"
            );
            return Err(anyhow!(
                "Kratos DELETE identity request failed with the status code `{response_status}`."
            ));
        }

        Ok(())
    }

    /// Tries to parse JWT and extract claims.
    async fn get_jwt_claims(&self, token: &str) -> anyhow::Result<Claims> {
        let Some(jwt_secret) = self.api.config.security.jwt_secret.as_ref() else {
            return Err(anyhow!("JWT secret is not configured."));
        };
        Ok(decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        )?
        .claims)
    }

    /// Updates user's subscription.
    pub async fn update_subscription(
        &self,
        user_email: &str,
        subscription: UserSubscription,
    ) -> anyhow::Result<Option<User>> {
        // Retrieve the user to combine new credentials with existing ones.
        let Some(mut existing_user) = self
            .api
            .users()
            .get_by_email(&user_email)
            .await
            .with_context(|| "Failed to retrieve user for subscription change.")?
        else {
            return Ok(None);
        };

        existing_user.subscription = subscription;

        // Update user with new subscription.
        self.api
            .db
            .upsert_user(&existing_user)
            .await
            .with_context(|| format!("Cannot update user ({})", *existing_user.id))?;

        Ok(Some(existing_user))
    }

    /// Generates a random user handle (8 bytes).
    pub async fn generate_user_handle(&self) -> anyhow::Result<String> {
        let mut bytes = [0u8; USER_HANDLE_LENGTH_BYTES];
        loop {
            getrandom::fill(&mut bytes)?;
            let handle = bytes.encode_hex::<String>();
            if self.api.users().get_by_handle(&handle).await?.is_none() {
                return Ok(handle);
            }
        }
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET>
where
    ET::Error: EmailTransportError,
{
    /// Returns an API to work with security related tasks.
    pub fn security(&self) -> SecurityApiExt<'_, DR, ET> {
        SecurityApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        security::credentials::Credentials,
        tests::{mock_api, mock_api_with_config, mock_config, mock_user},
        users::{ApiKeyCreateParams, SubscriptionTier, UserSubscription},
    };
    use actix_web::cookie::Cookie;
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;
    use sqlx::PgPool;
    use std::collections::HashSet;
    use time::OffsetDateTime;
    use url::Url;

    fn mock_identity_json(user_id: &str, email: &str) -> serde_json::Value {
        json!({
            "id": user_id,
            "traits": { "email": email },
            "verifiable_addresses": [{ "value": email, "verified": true }],
            "created_at": "2010-01-01T11:00:00Z"
        })
    }

    fn mock_session_json(user_id: &str, email: &str) -> serde_json::Value {
        json!({
            "id": "00000000-0000-0000-0000-000000000099",
            "identity": mock_identity_json(user_id, email)
        })
    }

    const TEST_JWT_SECRET: &str = "test-jwt-secret";

    fn encode_test_jwt(sub: &str, secret: &str) -> anyhow::Result<String> {
        let claims = json!({
            "sub": sub,
            "exp": (OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp()
        });
        Ok(encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )?)
    }

    #[sqlx::test]
    async fn properly_signs_user_up(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let user = mock_user()?;
        api.security().signup(&user).await?;

        let stored_user = api.users().get(user.id).await?.unwrap();
        assert_eq!(stored_user, user);

        Ok(())
    }

    #[sqlx::test]
    async fn cannot_signup_user_twice(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let security_api = api.security();

        let user = mock_user()?;
        api.security().signup(&user).await?;

        let signup_error = security_api.signup(&user).await.unwrap_err();
        assert_debug_snapshot!(signup_error, @"EmailAlreadyRegistered");

        let new_user = api.users().get(user.id).await?.unwrap();
        assert_eq!(new_user, user);

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_subscription(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let security_api = api.security();

        let user = mock_user()?;
        api.security().signup(&user).await?;
        assert_eq!(api.users().get(user.id).await?.unwrap(), user);

        let updated_user = security_api
            .update_subscription(
                &user.email,
                UserSubscription {
                    tier: SubscriptionTier::Standard,
                    ..user.subscription
                },
            )
            .await?
            .unwrap();
        assert_eq!(
            updated_user.subscription,
            UserSubscription {
                tier: SubscriptionTier::Standard,
                ..user.subscription
            }
        );
        assert_eq!(api.users().get(user.id).await?.unwrap(), updated_user);

        Ok(())
    }

    #[sqlx::test]
    async fn doesnt_throw_if_user_does_not_exist_for_subscription_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let security_api = api.security();

        let updated_user = security_api
            .update_subscription(
                "dev@secutils.dev",
                UserSubscription {
                    tier: SubscriptionTier::Standard,
                    started_at: OffsetDateTime::now_utc(),
                    ends_at: None,
                    trial_started_at: None,
                    trial_ends_at: None,
                },
            )
            .await?;
        assert!(updated_user.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_generate_user_handle(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let handle = api.security().generate_user_handle().await?;
        assert_eq!(handle.len(), super::USER_HANDLE_LENGTH_BYTES * 2);
        assert!(handle.chars().all(|c| c.is_ascii_hexdigit()));
        Ok(())
    }

    #[sqlx::test]
    async fn can_authenticate_with_session_cookie(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/sessions/whoami");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(mock_session_json(
                    &mock_user.id.to_string(),
                    &mock_user.email,
                ));
        });

        let mut config = mock_config()?;
        config.components.kratos_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let cookie = Cookie::new("id", "test-session-value");
        let user = api
            .security()
            .authenticate(&Credentials::SessionCookie(cookie))
            .await?
            .unwrap();
        assert_eq!(user.email, mock_user.email);
        assert_eq!(*user.id, *mock_user.id);
        assert!(user.is_activated);

        Ok(())
    }

    #[sqlx::test]
    async fn returns_none_for_invalid_session_cookie(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/sessions/whoami");
            then.status(401);
        });

        let mut config = mock_config()?;
        config.components.kratos_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let cookie = Cookie::new("id", "invalid-session");
        let result = api
            .security()
            .authenticate(&Credentials::SessionCookie(cookie))
            .await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_authenticate_with_jwt(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let token = encode_test_jwt(&mock_user.email, TEST_JWT_SECRET)?;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([mock_identity_json(
                    &mock_user.id.to_string(),
                    &mock_user.email,
                )]));
        });

        let mut config = mock_config()?;
        config.security.jwt_secret = Some(TEST_JWT_SECRET.to_string());
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let user = api
            .security()
            .authenticate(&Credentials::Jwt(token))
            .await?
            .unwrap();
        assert_eq!(user.email, mock_user.email);
        assert_eq!(*user.id, *mock_user.id);
        assert!(user.is_activated);

        Ok(())
    }

    #[sqlx::test]
    async fn fails_to_authenticate_with_jwt_if_secret_not_configured(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let result = api
            .security()
            .authenticate(&Credentials::Jwt("some-token".to_string()))
            .await;
        assert_debug_snapshot!(result.unwrap_err(), @r###""JWT secret is not configured.""###);

        Ok(())
    }

    #[sqlx::test]
    async fn can_authenticate_with_api_key(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;

        let server = MockServer::start();
        let user_id_str = mock_user.id.to_string();
        let user_email = mock_user.email.clone();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/admin/identities/{user_id_str}"));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(mock_identity_json(&user_id_str, &user_email));
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;
        api.db.upsert_user(&mock_user).await?;

        let (_key, plaintext) = api
            .api_keys(&mock_user)
            .create_api_key(ApiKeyCreateParams {
                name: "test-key".into(),
                expires_at: None,
            })
            .await?;

        let user = api
            .security()
            .authenticate(&Credentials::ApiKey(plaintext))
            .await?
            .unwrap();
        assert_eq!(user.email, mock_user.email);
        assert_eq!(*user.id, *mock_user.id);

        Ok(())
    }

    #[sqlx::test]
    async fn returns_none_for_invalid_api_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let result = api
            .security()
            .authenticate(&Credentials::ApiKey("su_ak_invalid_token".to_string()))
            .await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_terminate_user(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;

        let server = MockServer::start();
        let user_id_str = mock_user.id.to_string();
        let user_email = mock_user.email.clone();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([mock_identity_json(&user_id_str, &user_email)]));
        });
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/admin/identities/{user_id_str}"));
            then.status(204);
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let terminated_id = api.security().terminate(&mock_user.email).await?.unwrap();
        assert_eq!(terminated_id, mock_user.id);
        assert!(api.users().get(mock_user.id).await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_create_identity(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let new_id = "11111111-1111-1111-1111-111111111111";
        let new_email = "clone@secutils.dev";
        server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/identities");
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(mock_identity_json(new_id, new_email));
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let identity = api.security().create_identity(new_email, true).await?;
        assert_eq!(identity.id.to_string(), new_id);
        assert_eq!(identity.traits.email, new_email);
        assert!(identity.is_activated());

        Ok(())
    }

    #[sqlx::test]
    async fn create_identity_propagates_kratos_error(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/identities");
            then.status(409).body("identity already exists");
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let result = api
            .security()
            .create_identity("dup@secutils.dev", true)
            .await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test]
    async fn can_create_recovery_link(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let identity_id = uuid::uuid!("22222222-2222-2222-2222-222222222222");
        let expected_link =
            "http://127.0.0.1:7171/self-service/recovery?flow=abc&token=xyz".to_string();
        server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/recovery/code");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "recovery_link": expected_link,
                    "recovery_code": "123456",
                    "expires_at": "2030-01-01T10:00:00Z"
                }));
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let link = api
            .security()
            .create_recovery_link(identity_id, "1h")
            .await?;
        assert_eq!(link.recovery_link, expected_link);
        assert_eq!(link.recovery_code.as_deref(), Some("123456"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_recovery_link_propagates_kratos_error(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/recovery/code");
            then.status(400).body("invalid identity");
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let result = api
            .security()
            .create_recovery_link(uuid::Uuid::new_v4(), "1h")
            .await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test]
    async fn clone_data_round_trips_secrets_through_ephemeral_passphrase(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        use crate::{tests::mock_user_with_id, users::SecretCreateParams};

        let mut config = mock_config()?;
        // Required so `secrets(user)` can encrypt/decrypt server-side.
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        // `clone_data` calls `generate_export` which unconditionally queries Retrack for the
        // user's page/api trackers - point it at a local mock that returns an empty list so
        // the test doesn't depend on a live Retrack (the source has no trackers anyway).
        let retrack_server = MockServer::start();
        retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([]));
        });
        config.retrack.host = Url::parse(&retrack_server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let source = mock_user_with_id(uuid::uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))?;
        let destination = mock_user_with_id(uuid::uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"))?;
        api.db.insert_user(&source).await?;
        api.db.insert_user(&destination).await?;

        api.secrets(&source)
            .create_secret(SecretCreateParams {
                name: "MY_KEY".to_string(),
                value: "super-secret-value".to_string(),
                tag_ids: vec![],
            })
            .await?;

        let summary = api
            .security()
            .clone_data(&source, &destination, false)
            .await?;
        assert_eq!(summary.results.secrets.imported, 1);
        assert_eq!(summary.results.secrets.failed, 0);

        // Verify the destination's copy decrypts back to the original plaintext (proves the
        // ephemeral passphrase round-trip actually re-encrypted the value under the global key).
        let decrypted = api.secrets(&destination).decrypt_all_secrets().await?;
        let cloned_value = decrypted
            .get("MY_KEY")
            .expect("MY_KEY should be present in the destination");
        assert_eq!(cloned_value, "super-secret-value");

        Ok(())
    }

    #[sqlx::test]
    async fn terminate_returns_none_for_nonexistent_user(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([]));
        });

        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;

        let result = api.security().terminate("nonexistent@secutils.dev").await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_operator_with_jwt(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let token = encode_test_jwt(&mock_user.email, TEST_JWT_SECRET)?;

        let mut config = mock_config()?;
        config.security.jwt_secret = Some(TEST_JWT_SECRET.to_string());
        config.security.operators = Some(HashSet::from([mock_user.email.clone()]));
        let api = mock_api_with_config(pool, config).await?;

        let operator = api
            .security()
            .get_operator(&Credentials::Jwt(token))
            .await?
            .unwrap();
        assert_eq!(operator.id(), mock_user.email);

        Ok(())
    }

    #[sqlx::test]
    async fn get_operator_returns_none_for_non_operator_jwt(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let token = encode_test_jwt(&mock_user.email, TEST_JWT_SECRET)?;

        let mut config = mock_config()?;
        config.security.jwt_secret = Some(TEST_JWT_SECRET.to_string());
        config.security.operators = Some(HashSet::from(["other@secutils.dev".to_string()]));
        let api = mock_api_with_config(pool, config).await?;

        let result = api
            .security()
            .get_operator(&Credentials::Jwt(token))
            .await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn get_operator_returns_none_for_api_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let result = api
            .security()
            .get_operator(&Credentials::ApiKey("su_ak_any_token".to_string()))
            .await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_operator_with_session_cookie(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/sessions/whoami");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(mock_session_json(
                    &mock_user.id.to_string(),
                    &mock_user.email,
                ));
        });

        let mut config = mock_config()?;
        config.components.kratos_url = Url::parse(&server.base_url())?;
        config.security.operators = Some(HashSet::from([mock_user.email.clone()]));
        let api = mock_api_with_config(pool, config).await?;

        let cookie = Cookie::new("id", "test-session-value");
        let operator = api
            .security()
            .get_operator(&Credentials::SessionCookie(cookie))
            .await?
            .unwrap();
        assert_eq!(operator.id(), mock_user.email);

        Ok(())
    }

    #[sqlx::test]
    async fn authenticate_sets_is_operator_flag(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let token = encode_test_jwt(&mock_user.email, TEST_JWT_SECRET)?;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([mock_identity_json(
                    &mock_user.id.to_string(),
                    &mock_user.email,
                )]));
        });

        let mut config = mock_config()?;
        config.security.jwt_secret = Some(TEST_JWT_SECRET.to_string());
        config.security.operators = Some(HashSet::from([mock_user.email.clone()]));
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let user = api
            .security()
            .authenticate(&Credentials::Jwt(token))
            .await?
            .unwrap();
        assert!(user.is_operator);

        Ok(())
    }

    #[sqlx::test]
    async fn authenticate_returns_none_when_user_not_in_db(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let token = encode_test_jwt(&mock_user.email, TEST_JWT_SECRET)?;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([mock_identity_json(
                    &mock_user.id.to_string(),
                    &mock_user.email,
                )]));
        });

        let mut config = mock_config()?;
        config.security.jwt_secret = Some(TEST_JWT_SECRET.to_string());
        config.components.kratos_admin_url = Url::parse(&server.base_url())?;
        let api = mock_api_with_config(pool, config).await?;
        // Do NOT insert user in DB

        let result = api
            .security()
            .authenticate(&Credentials::Jwt(token))
            .await?;
        assert!(result.is_none());

        Ok(())
    }
}
