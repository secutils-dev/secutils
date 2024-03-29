use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    notifications::{NotificationContent, NotificationContentTemplate, NotificationDestination},
    security::{
        Credentials, StoredCredentials, WebAuthnChallenge, WebAuthnChallengeType, WebAuthnSession,
        WebAuthnSessionValue,
    },
    users::{
        InternalUserDataNamespace, SubscriptionTier, User, UserData, UserId, UserSignupError,
        UserSubscription,
    },
};
use anyhow::{anyhow, bail, Context};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use hex::ToHex;
use rand_core::{OsRng, RngCore};
use std::{
    ops::{Add, Sub},
    time::Duration,
};
use time::OffsetDateTime;
use uuid::Uuid;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

const USER_HANDLE_LENGTH_BYTES: usize = 8;

const ACTIVATION_CODE_LENGTH_BYTES: usize = 32;
const CREDENTIALS_RESET_CODE_LENGTH_BYTES: usize = 32;

/// Activation code is valid for 14 days.
const ACTIVATION_CODE_LIFESPAN: Duration = Duration::from_secs(60 * 60 * 24 * 14);
/// Credentials reset code is valid for 1 hour.
const CREDENTIALS_RESET_CODE_LIFESPAN: Duration = Duration::from_secs(60 * 60);

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
    pub async fn signup<E: Into<String>>(
        &self,
        user_email: E,
        user_credentials: Credentials,
    ) -> anyhow::Result<User> {
        // Perform only basic checks here, consumer is supposed to properly validate all corner cases.
        let user_email = user_email.into();
        if user_email.is_empty() {
            bail!("Cannot signup user with invalid email: `{}`.", user_email);
        }

        // Check if the user with specified email already exists.
        if let Some(user) = self
            .api
            .users()
            .get_by_email(&user_email)
            .await
            .with_context(|| "Failed to check if user already exists.")?
        {
            log::error!(user:serde = user.log_context(); "User is already registered.");
            return Err(UserSignupError::EmailAlreadyRegistered.into());
        }

        let credentials = self
            .validate_credentials(&user_email, user_credentials)
            .await?;
        let created =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;

        // Signup user with a basic subscription by default and activate trial.
        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: created,
            ends_at: None,
            trial_started_at: Some(created),
            trial_ends_at: Some(created.add(UserSubscription::TRIAL_LENGTH)),
        };

        let user = User {
            id: UserId::default(),
            email: user_email,
            handle: self.generate_user_handle().await?,
            credentials,
            created,
            subscription,
            activated: false,
        };

        // Use insert instead of upsert here to prevent multiple signup requests from the same user.
        // Consumer of the API is supposed to perform validation before invoking this method.
        let user = self
            .api
            .db
            .insert_user(&user)
            .await
            .with_context(|| "Cannot signup user, failed to insert a new user.")
            .map(|user_id| User {
                id: user_id,
                ..user
            })?;

        // Send email to the user with the account activation link.
        self.send_activation_link(&user).await?;

        Ok(user)
    }

    /// Authenticates user with the specified email and password.
    pub async fn authenticate<E: AsRef<str>>(
        &self,
        user_email: E,
        user_credentials: Credentials,
    ) -> anyhow::Result<User> {
        let mut user =
            if let Some(user) = self.api.users().get_by_email(user_email.as_ref()).await? {
                user
            } else {
                bail!(
                    "Cannot authenticate user: a user with {} email doesn't exist.",
                    user_email.as_ref()
                );
            };

        match user_credentials {
            Credentials::Password(password) => {
                let parsed_hash = if let Some(ref password_hash) = user.credentials.password_hash {
                    PasswordHash::new(password_hash)
                        .map_err(|err| anyhow!("Failed to parse a password hash: {}", err))?
                } else {
                    bail!(
                        "Cannot authenticate user: a user with {} email doesn't have password-based credentials.",
                        user.email
                    );
                };

                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed_hash)
                    .map_err(|err| {
                        anyhow!(
                            "Cannot authenticate user: failed to validate password for user {} due to {}",
                            user.handle,
                            err
                        )
                    })?;
            }
            Credentials::WebAuthnPublicKey(serialized_public_key) => {
                let webauthn_session = self
                    .api
                    .db
                    .get_user_webauthn_session_by_email(&user.email)
                    .await?
                    .ok_or_else(|| anyhow!("Cannot find WebAuthn session in database."))?;

                // Make sure that WebAuthn session was created for authentication.
                let authentication_state = if let WebAuthnSessionValue::AuthenticationState(state) =
                    webauthn_session.value
                {
                    state
                } else {
                    bail!(
                        "WebAuthn session value isn't suitable for authentication: {:?}",
                        webauthn_session.value
                    );
                };

                // Deserialize public key and finish authentication.
                let authentication_result = self.api.webauthn.finish_passkey_authentication(
                    &serde_json::from_value::<PublicKeyCredential>(serialized_public_key)?,
                    &authentication_state,
                )?;

                // Update credentials counter to protect against cloned authenticators.
                if authentication_result.needs_update() {
                    if let Some(mut passkey) = user.credentials.passkey.take() {
                        passkey.update_credential(&authentication_result);
                        user.credentials.passkey = Some(passkey);

                        self.api
                            .users()
                            .upsert(&user)
                            .await
                            .with_context(|| "Couldn't update passkey credentials.")?;
                    } else {
                        bail!(
                            "Cannot authenticate user: a user with {} email doesn't have passkey credentials.",
                           user.email
                        );
                    }
                }

                // Clear WebAuthn session state since we no longer need it.
                self.api
                    .db
                    .remove_user_webauthn_session_by_email(&user.email)
                    .await?;
            }
        }

        Ok(user)
    }

    async fn validate_credentials(
        &self,
        user_email: &str,
        user_credentials: Credentials,
    ) -> anyhow::Result<StoredCredentials> {
        let user_credentials = match user_credentials {
            Credentials::Password(password) => StoredCredentials::try_from_password(&password)?,
            Credentials::WebAuthnPublicKey(serialized_public_key) => {
                let webauthn_session = self
                    .api
                    .db
                    .get_user_webauthn_session_by_email(user_email)
                    .await?
                    .ok_or_else(|| anyhow!("Cannot find WebAuthn session in database."))?;

                // Make sure that WebAuthn session was created for registration.
                let registration_state = if let WebAuthnSessionValue::RegistrationState(state) =
                    webauthn_session.value
                {
                    state
                } else {
                    bail!(
                        "WebAuthn session value isn't suitable for registration: {:?}",
                        webauthn_session.value
                    );
                };

                // Deserialize public key and finish registration and extract passkey.
                let credentials = self
                    .api
                    .webauthn
                    .finish_passkey_registration(
                        &serde_json::from_value::<RegisterPublicKeyCredential>(
                            serialized_public_key,
                        )?,
                        &registration_state,
                    )
                    .map(StoredCredentials::from_passkey)?;

                // Clear WebAuthn session state since we no longer need it.
                self.api
                    .db
                    .remove_user_webauthn_session_by_email(user_email)
                    .await?;

                credentials
            }
        };

        if user_credentials.is_empty() {
            bail!("User credentials are empty.");
        }

        Ok(user_credentials)
    }

    /// Adds or updates user credentials.
    pub async fn update_credentials(
        &self,
        user_email: &str,
        user_credentials: Credentials,
    ) -> anyhow::Result<User> {
        // Perform only basic checks here, consumer is supposed to properly validate all corner cases.
        if user_email.is_empty() {
            bail!(
                "Cannot update user credentials: invalid user email `{}`.",
                user_email
            );
        }

        // Retrieve the user to combine new credentials with existing ones.
        let mut existing_user = self
            .api
            .users()
            .get_by_email(&user_email)
            .await
            .with_context(|| "Failed to retrieve user for credentials change.")?
            .ok_or_else(|| anyhow!("User to change password for doesn't exist."))?;

        // Merge credentials
        let user_credentials = self
            .validate_credentials(user_email, user_credentials)
            .await?;
        if user_credentials.password_hash.is_some() {
            existing_user.credentials.password_hash = user_credentials.password_hash;
        } else if user_credentials.passkey.is_some() {
            existing_user.credentials.passkey = user_credentials.passkey;
        }

        // Update user with new credentials.
        self.api
            .users()
            .upsert(&existing_user)
            .await
            .with_context(|| format!("Cannot update user ({})", *existing_user.id))?;

        Ok(existing_user)
    }

    /// Resets user credentials using the specified email and credentials reset code.
    pub async fn reset_credentials(
        &self,
        user_email: &str,
        user_credentials: Credentials,
        user_reset_credentials_code: &str,
    ) -> anyhow::Result<User> {
        // First check if user with the specified email exists.
        let user_to_reset_credentials = self
            .api
            .users()
            .get_by_email(user_email)
            .await?
            .ok_or_else(|| {
                anyhow!(
                "User with the specified email doesn't exist. Credentials reset isn't possible."
            )
            })?;

        // Then, try to retrieve reset code.
        let reset_code = self.api.users().get_data::<String>(
                user_to_reset_credentials.id,
                InternalUserDataNamespace::CredentialsResetToken,
            )
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "User doesn't have assigned credentials reset code. Credentials reset isn't possible."
                )
            })?;

        if reset_code.value != user_reset_credentials_code {
            bail!("User credentials reset code is not valid.");
        }

        if reset_code.timestamp < OffsetDateTime::now_utc().sub(CREDENTIALS_RESET_CODE_LIFESPAN) {
            bail!(
                "User credentials rest code has expired (created on {}).",
                reset_code.timestamp
            );
        }

        // Update credentials and invalid credentials reset code.
        self.update_credentials(user_email, user_credentials)
            .await?;
        self.api
            .users()
            .remove_data(
                user_to_reset_credentials.id,
                InternalUserDataNamespace::CredentialsResetToken,
            )
            .await?;

        Ok(user_to_reset_credentials)
    }

    /// Activates user using the specified email and activation code.
    pub async fn activate(
        &self,
        user_email: &str,
        user_activation_code: &str,
    ) -> anyhow::Result<User> {
        // First check if user with the specified email exists.
        let users = self.api.users();
        let mut user_to_activate = users.get_by_email(user_email).await?.with_context(|| {
            "User with the specified email doesn't exist. Account activation isn't possible."
        })?;

        // Then, try to retrieve activation code.
        let activation_code = users
            .get_data::<String>(
                user_to_activate.id,
                InternalUserDataNamespace::AccountActivationToken,
            )
            .await?
            .with_context(|| {
                "User doesn't have assigned activation code. Account activation isn't possible."
            })?;

        if activation_code.value != user_activation_code {
            bail!("User activation code is not valid.");
        }

        if activation_code.timestamp < OffsetDateTime::now_utc().sub(ACTIVATION_CODE_LIFESPAN) {
            bail!(
                "User activation code has expired (created on {}).",
                activation_code.timestamp
            );
        }

        // Update user and remove activation code internal data.
        user_to_activate.activated = true;
        users.upsert(&user_to_activate).await.with_context(|| {
            format!(
                "Cannot activate user: failed to store activated user {}",
                user_to_activate.handle
            )
        })?;
        users
            .remove_data(
                user_to_activate.id,
                InternalUserDataNamespace::AccountActivationToken,
            )
            .await?;

        Ok(user_to_activate)
    }

    /// Generates a new account activation link for the specified user and sends to the user's email.
    pub async fn send_activation_link(&self, user: &User) -> anyhow::Result<()> {
        let mut bytes = [0u8; ACTIVATION_CODE_LENGTH_BYTES];
        OsRng.fill_bytes(&mut bytes);
        let activation_code: String = bytes.encode_hex();

        let timestamp = OffsetDateTime::now_utc();
        let namespace = InternalUserDataNamespace::AccountActivationToken;

        // Cleanup already expired activation codes.
        self.api
            .db
            .cleanup_user_data(namespace, timestamp.sub(ACTIVATION_CODE_LIFESPAN))
            .await
            .with_context(|| "Failed to cleanup expired activation codes.")?;

        // Save newly created activation code.
        self.api
            .db
            .upsert_user_data(
                namespace,
                UserData::new(user.id, &activation_code, timestamp),
            )
            .await
            .with_context(|| {
                format!("Cannot store activation code for the user ({}).", *user.id)
            })?;

        // Schedule a email notification that will include activation link.
        self.api
            .notifications()
            .schedule_notification(
                NotificationDestination::User(user.id),
                NotificationContent::Template(NotificationContentTemplate::AccountActivation {
                    user_id: user.id,
                }),
                OffsetDateTime::now_utc(),
            )
            .await?;

        Ok(())
    }

    /// Generates credentials reset link for the specified user and sends to the user's email.
    pub async fn send_credentials_reset_link(&self, user: &User) -> anyhow::Result<()> {
        let mut bytes = [0u8; CREDENTIALS_RESET_CODE_LENGTH_BYTES];
        OsRng.fill_bytes(&mut bytes);
        let reset_code: String = bytes.encode_hex();

        let timestamp = OffsetDateTime::now_utc();
        let namespace = InternalUserDataNamespace::CredentialsResetToken;

        // Cleanup already expired codes.
        self.api
            .db
            .cleanup_user_data(namespace, timestamp.sub(CREDENTIALS_RESET_CODE_LIFESPAN))
            .await
            .with_context(|| "Failed to cleanup expired credentials reset codes.")?;

        // Save newly created credentials reset code.
        self.api
            .db
            .upsert_user_data(namespace, UserData::new(user.id, &reset_code, timestamp))
            .await
            .with_context(|| {
                format!(
                    "Cannot store credentials reset code for the user ({}).",
                    *user.id
                )
            })?;

        // Schedule a email notification that will include password reset link.
        self.api
            .notifications()
            .schedule_notification(
                NotificationDestination::User(user.id),
                NotificationContent::Template(NotificationContentTemplate::PasswordReset {
                    user_id: user.id,
                }),
                OffsetDateTime::now_utc(),
            )
            .await?;

        Ok(())
    }

    /// Starts WebAuthn handshake by generating a challenge of the specified type for the specified
    /// user email, and storing WebAuthn session in the database. The result should be returned to
    /// the user's browser for processing only. This operation also triggers cleanup of the stale
    /// WebAuthn session from the database.
    pub async fn start_webauthn_handshake(
        &self,
        user_email: &str,
        challenge_type: WebAuthnChallengeType,
    ) -> anyhow::Result<WebAuthnChallenge> {
        // Clean up sessions that are older than 10 minutes, based on the recommended timeout values
        // suggested in the WebAuthn spec: https://www.w3.org/TR/webauthn-2/#sctn-createCredential.
        self.api
            .db
            .remove_user_webauthn_sessions(
                OffsetDateTime::now_utc().sub(Duration::from_secs(60 * 10)),
            )
            .await
            .with_context(|| "Failed to cleanup stale WebAuthn session.")?;

        // Generate challenge response.
        let (challenge, webauthn_session_value) = match challenge_type {
            WebAuthnChallengeType::Registration => {
                let (ccr, reg_state) = self
                    .api
                    .webauthn
                    .start_passkey_registration(Uuid::new_v4(), user_email, user_email, None)
                    .with_context(|| "Failed to start passkey registration")?;

                (
                    WebAuthnChallenge::registration(&ccr)?,
                    WebAuthnSessionValue::RegistrationState(reg_state),
                )
            }
            WebAuthnChallengeType::Authentication => {
                // Make sure user with specified email exists.
                let user = self
                    .api
                    .users()
                    .get_by_email(user_email)
                    .await?
                    .ok_or_else(|| anyhow!("User is not found (`{}`).", user_email))?;

                // Make sure the user has passkey credentials configured.
                let passkey = user
                    .credentials
                    .passkey
                    .ok_or_else(|| anyhow!("User doesn't have a passkey configured."))?;

                let (ccr, auth_state) = self
                    .api
                    .webauthn
                    .start_passkey_authentication(&[passkey])
                    .with_context(|| "Failed to start passkey authentication")?;

                (
                    WebAuthnChallenge::authentication(&ccr)?,
                    WebAuthnSessionValue::AuthenticationState(auth_state),
                )
            }
        };

        // Store WebAuthn session state in the database during handshake.
        self.api
            .db
            .upsert_user_webauthn_session(&WebAuthnSession {
                email: user_email.to_string(),
                value: webauthn_session_value,
                timestamp: OffsetDateTime::now_utc(),
            })
            .await?;

        Ok(challenge)
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
    async fn generate_user_handle(&self) -> anyhow::Result<String> {
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
        notifications::{
            NotificationContent, NotificationContentTemplate, NotificationDestination,
        },
        security::Credentials,
        tests::mock_api,
        users::{SubscriptionTier, UserSubscription},
    };
    use insta::assert_debug_snapshot;
    use std::ops::Add;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn properly_signs_user_up() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let security_api = api.security();

        let user = security_api
            .signup(
                "dev@secutils.dev",
                Credentials::Password("pass".to_string()),
            )
            .await?;

        assert_eq!(user.email, "dev@secutils.dev");
        assert!(user.credentials.password_hash.is_some());
        assert!(!user.activated);
        assert_eq!(
            user.subscription,
            UserSubscription {
                tier: SubscriptionTier::Basic,
                started_at: user.created,
                ends_at: None,
                trial_started_at: Some(user.created),
                trial_ends_at: Some(user.created.add(UserSubscription::TRIAL_LENGTH)),
            }
        );

        let activation_notification = api.db.get_notification(1.try_into()?).await?.unwrap();
        assert_eq!(
            activation_notification.destination,
            NotificationDestination::User(user.id)
        );
        assert_eq!(
            activation_notification.content,
            NotificationContent::Template(NotificationContentTemplate::AccountActivation {
                user_id: user.id
            })
        );

        Ok(())
    }

    #[tokio::test]
    async fn cannot_signup_user_twice() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let security_api = api.security();

        let user = security_api
            .signup(
                "dev@secutils.dev",
                Credentials::Password("pass".to_string()),
            )
            .await?;

        let signup_error = security_api
            .signup(
                "dev@secutils.dev",
                Credentials::Password("pass".to_string()),
            )
            .await
            .unwrap_err();
        assert_debug_snapshot!(signup_error, @"EmailAlreadyRegistered");

        let new_user = api.users().get(user.id).await?.unwrap();
        assert_eq!(new_user, user);

        Ok(())
    }

    #[tokio::test]
    async fn can_update_subscription() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let security_api = api.security();

        let user = security_api
            .signup(
                "dev@secutils.dev",
                Credentials::Password("pass".to_string()),
            )
            .await?;
        assert_eq!(
            user.subscription,
            UserSubscription {
                tier: SubscriptionTier::Basic,
                started_at: user.created,
                ends_at: None,
                trial_started_at: Some(user.created),
                trial_ends_at: Some(user.created.add(UserSubscription::TRIAL_LENGTH)),
            }
        );
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

    #[tokio::test]
    async fn doesnt_throw_if_user_does_not_exist_for_subscription_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
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
