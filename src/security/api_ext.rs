use crate::{
    api::Api,
    logging::UserLogContext,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    security::{
        credentials::Credentials,
        jwt::Claims,
        kratos::{Identity, Session},
        Operator,
    },
    users::{User, UserId, UserSignupError, UserSubscription},
};
use anyhow::{anyhow, Context};
use hex::ToHex;
use jsonwebtoken::{decode, DecodingKey, Validation};
use rand_core::{OsRng, RngCore};
use reqwest::StatusCode;

const USER_HANDLE_LENGTH_BYTES: usize = 8;

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
    /// activation. After signup, we'll send email with the activation code, and will re-send it
    /// after 7 days, then after 14 days, and after 30 days we'll terminate account with a large
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
            log::error!(user:serde = user.log_context(); "User is already registered.");
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

    /// Authenticates user with the specified credentials.
    pub async fn authenticate(&self, credentials: Credentials) -> anyhow::Result<Option<User>> {
        let Some(identity) = self.get_identity(&credentials).await? else {
            log::error!(
                "Couldn't retrieve user identity with `{}` credentials.",
                match credentials {
                    Credentials::SessionCookie(_) => "session",
                    Credentials::Jwt(_) => "jwt",
                }
            );
            return Ok(None);
        };

        let Some(user) = self.api.users().get(UserId::from(identity.id)).await? else {
            log::error!(
                user:serde = UserLogContext::new(UserId::from(identity.id));
                "User doesn't exist."
            );
            return Ok(None);
        };

        Ok(Some(User {
            created_at: identity.created_at,
            activated: identity.activated(),
            ..user
        }))
    }

    /// Checks if the user or service account with specified credentials is an operator.
    pub async fn get_operator(&self, credentials: Credentials) -> anyhow::Result<Option<Operator>> {
        let operator_id = match &credentials {
            // If the user is authenticated with a session cookie, user's email is used as an
            // operator identifier.
            Credentials::SessionCookie(_) => {
                self.get_identity(&credentials)
                    .await?
                    .ok_or_else(|| anyhow!("Session cookie is invalid"))?
                    .traits
                    .email
            }
            // For JWT, we treat `sub` claim as an operator identifier.
            Credentials::Jwt(token) => self.get_jwt_claims(token).await?.sub,
        };

        let operators = self.api.config.security.operators.as_ref();
        if operators.map_or(false, |operators| operators.contains(&operator_id)) {
            Ok(Some(Operator::new(operator_id)))
        } else {
            Ok(None)
        }
    }

    /// Tries to retrieve user identity from Kratos using specified credentials.
    async fn get_identity(&self, credentials: &Credentials) -> anyhow::Result<Option<Identity>> {
        let client = reqwest::Client::new();
        let request_builder = match credentials {
            Credentials::SessionCookie(cookie) => client
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
                ),
            Credentials::Jwt(token) => {
                let claims = self.get_jwt_claims(token).await?;
                client.request(
                    reqwest::Method::GET,
                    format!(
                        "{}admin/identities?credentials_identifier={}",
                        self.api.config.components.kratos_admin_url.as_str(),
                        urlencoding::encode(&claims.sub)
                    ),
                )
            }
        };

        let request = match request_builder.build() {
            Ok(client) => client,
            Err(err) => {
                log::error!("Cannot build Kratos request: {err:?}");
                return Err(anyhow!(err));
            }
        };

        let response = match client.execute(request).await {
            Ok(response) => response,
            Err(err) => {
                log::error!("Cannot execute Kratos request: {err:?}");
                return Err(anyhow!(err));
            }
        };

        let response_status = response.status();
        if !response_status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return match response_status {
                StatusCode::UNAUTHORIZED => {
                    log::error!("Kratos request couldn't be authenticated: {error_text}");
                    Ok(None)
                }
                _ => {
                    log::error!(
                        "Kratos request failed with the status code `{response_status}` and body: {error_text}"
                    );
                    Err(anyhow!(
                        "Kratos request failed with the status code `{response_status}`."
                    ))
                }
            };
        }

        Ok(match credentials {
            Credentials::SessionCookie(_) => response
                .json::<Session>()
                .await
                .map(|session| session.identity)?,
            Credentials::Jwt(_) => response
                .json::<Vec<Identity>>()
                .await
                .map(|identities| identities.into_iter().next())?,
        })
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
            .users()
            .upsert(&existing_user)
            .await
            .with_context(|| format!("Cannot update user ({})", *existing_user.id))?;

        Ok(Some(existing_user))
    }

    /// Generates a random user handle (8 bytes).
    pub async fn generate_user_handle(&self) -> anyhow::Result<String> {
        let mut bytes = [0u8; USER_HANDLE_LENGTH_BYTES];
        loop {
            OsRng.fill_bytes(&mut bytes);
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
    pub fn security(&self) -> SecurityApiExt<DR, ET> {
        SecurityApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api, mock_user},
        users::{SubscriptionTier, UserSubscription},
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use time::OffsetDateTime;

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
}
