use crate::{
    api::{
        users::{DictionaryDataUserDataSetter, UserDataSetter, UserSignupError},
        Email, EmailBody, EmailsApi,
    },
    authentication::{
        Credentials, StoredCredentials, WebAuthnChallenge, WebAuthnChallengeType, WebAuthnSession,
        WebAuthnSessionValue,
    },
    config::Config,
    datastore::PrimaryDb,
    users::{
        BuiltinUser, InternalUserDataNamespace, PublicUserDataNamespace, User, UserData,
        UserDataKey, UserDataNamespace, UserId, UserSettingsSetter,
    },
    utils::{AutoResponder, ContentSecurityPolicy, SelfSignedCertificate},
};
use anyhow::{anyhow, bail, Context};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use hex::ToHex;
use rand_core::{OsRng, RngCore};
use serde::de::DeserializeOwned;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    ops::Sub,
    time::{Duration, SystemTime},
};
use time::OffsetDateTime;
use uuid::Uuid;
use webauthn_rs::{
    prelude::{PublicKeyCredential, RegisterPublicKeyCredential},
    Webauthn,
};

const USER_HANDLE_LENGTH_BYTES: usize = 8;
const ACTIVATION_CODE_LENGTH_BYTES: usize = 32;
const CREDENTIALS_RESET_CODE_LENGTH_BYTES: usize = 32;

/// Activation code is valid for 14 days.
const ACTIVATION_CODE_LIFESPAN: Duration = Duration::from_secs(60 * 60 * 24 * 14);
/// Credentials reset code is valid for 1 hour.
const CREDENTIALS_RESET_CODE_LIFESPAN: Duration = Duration::from_secs(60 * 60);

pub struct UsersApi<'a> {
    config: &'a Config,
    webauthn: &'a Webauthn,
    emails: EmailsApi<&'a Config>,
    primary_db: Cow<'a, PrimaryDb>,
}

impl<'a> UsersApi<'a> {
    /// Creates Users API.
    pub fn new(
        config: &'a Config,
        webauthn: &'a Webauthn,
        primary_db: &'a PrimaryDb,
        emails: EmailsApi<&'a Config>,
    ) -> Self {
        Self {
            config,
            webauthn,
            emails,
            primary_db: Cow::Borrowed(primary_db),
        }
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
            bail!("Cannot signup user: invalid user email `{}`.", user_email);
        }

        // Check if the user with specified email already exists.
        if let Some(user) = self
            .get_by_email(&user_email)
            .await
            .with_context(|| "Failed to check if user already exists.")?
        {
            log::error!("User is already registered (user ID: {:?}).", user.id);
            return Err(UserSignupError::EmailAlreadyRegistered.into());
        }

        let credentials = self
            .validate_credentials(&user_email, user_credentials)
            .await?;
        let user = User {
            id: UserId::empty(),
            email: user_email,
            handle: self.generate_user_handle().await?,
            credentials,
            created: OffsetDateTime::now_utc(),
            roles: HashSet::with_capacity(0),
            activated: false,
        };

        // Use insert instead of upsert here to prevent multiple signup requests from the same user.
        // Consumer of the API is supposed to perform validation before invoking this method.
        let user = self
            .primary_db
            .insert_user(&user)
            .await
            .with_context(|| "Cannot signup user, failed to insert a new user.")
            .map(|user_id| User {
                id: user_id,
                ..user
            })?;

        // Send an email to the user with the account activation link.
        self.send_activation_link(&user).await?;

        Ok(user)
    }

    /// Authenticates user with the specified email and password.
    pub async fn authenticate<E: AsRef<str>>(
        &self,
        user_email: E,
        user_credentials: Credentials,
    ) -> anyhow::Result<User> {
        let mut user = if let Some(user) = self
            .primary_db
            .get_user_by_email(user_email.as_ref())
            .await?
        {
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
                    .primary_db
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
                let authentication_result = self.webauthn.finish_passkey_authentication(
                    &serde_json::from_value::<PublicKeyCredential>(serialized_public_key)?,
                    &authentication_state,
                )?;

                // Update credentials counter to protect against cloned authenticators.
                if authentication_result.needs_update() {
                    if let Some(mut passkey) = user.credentials.passkey.take() {
                        passkey.update_credential(&authentication_result);
                        user.credentials.passkey = Some(passkey);

                        self.upsert(&user)
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
                self.primary_db
                    .remove_user_webauthn_session_by_email(&user.email)
                    .await?;
            }
        }

        Ok(user)
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
        self.primary_db
            .upsert_user(&existing_user)
            .await
            .with_context(|| format!("Cannot update user (user ID: {:?})", existing_user.id))?;

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
        let user_to_reset_credentials = self.get_by_email(user_email).await?.ok_or_else(|| {
            anyhow!(
                "User with the specified email doesn't exist. Credentials reset isn't possible."
            )
        })?;

        // Then, try to retrieve reset code.
        let reset_code = self
            .get_data::<String>(
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
        self.primary_db
            .remove_user_data(
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
        let mut user_to_activate = self.get_by_email(user_email).await?.ok_or_else(|| {
            anyhow!(
                "User with the specified email doesn't exist. Account activation isn't possible."
            )
        })?;

        // Then, try to retrieve activation code.
        let activation_code = self
            .get_data::<String>(
                user_to_activate.id,
                InternalUserDataNamespace::AccountActivationToken,
            )
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "User doesn't have assigned activation code. Account activation isn't possible."
                )
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
        self.primary_db
            .upsert_user(&user_to_activate)
            .await
            .with_context(|| {
                format!(
                    "Cannot activate user: failed to store activated user {}",
                    user_to_activate.handle
                )
            })?;
        self.primary_db
            .remove_user_data(
                user_to_activate.id,
                InternalUserDataNamespace::AccountActivationToken,
            )
            .await?;

        Ok(user_to_activate)
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
        self.primary_db
            .remove_user_webauthn_sessions(
                OffsetDateTime::now_utc().sub(Duration::from_secs(60 * 10)),
            )
            .await
            .with_context(|| "Failed to cleanup stale WebAuthn session.")?;

        // Generate challenge response.
        let (challenge, webauthn_session_value) = match challenge_type {
            WebAuthnChallengeType::Registration => {
                let (ccr, reg_state) = self
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
                    .get_by_email(user_email)
                    .await?
                    .ok_or_else(|| anyhow!("User is not found (`{}`).", user_email))?;

                // Make sure the user has passkey credentials configured.
                let passkey = user
                    .credentials
                    .passkey
                    .ok_or_else(|| anyhow!("User doesn't have a passkey configured."))?;

                let (ccr, auth_state) = self
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
        self.primary_db
            .upsert_user_webauthn_session(&WebAuthnSession {
                email: user_email.to_string(),
                value: webauthn_session_value,
                timestamp: OffsetDateTime::now_utc(),
            })
            .await?;

        Ok(challenge)
    }

    /// Retrieves the user using the specified email.
    pub async fn get_by_email<E: AsRef<str>>(&self, user_email: E) -> anyhow::Result<Option<User>> {
        self.primary_db.get_user_by_email(user_email).await
    }

    /// Retrieves the user using the specified handle.
    pub async fn get_by_handle<E: AsRef<str>>(
        &self,
        user_handle: E,
    ) -> anyhow::Result<Option<User>> {
        self.primary_db.get_user_by_handle(user_handle).await
    }

    /// Inserts or updates user in the `Users` store.
    pub async fn upsert<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        self.primary_db.upsert_user(user).await
    }

    /// Inserts or updates user in the `Users` store using `BuiltinUser`.
    pub async fn upsert_builtin(&self, builtin_user: BuiltinUser) -> anyhow::Result<UserId> {
        let user = match self
            .primary_db
            .get_user_by_email(&builtin_user.email)
            .await?
        {
            Some(user) => User {
                id: user.id,
                email: user.email,
                handle: user.handle,
                created: user.created,
                credentials: builtin_user.credentials,
                roles: builtin_user.roles,
                activated: true,
            },
            None => User {
                id: UserId::empty(),
                email: builtin_user.email,
                handle: self.generate_user_handle().await?,
                credentials: builtin_user.credentials,
                created: OffsetDateTime::now_utc(),
                roles: builtin_user.roles,
                activated: true,
            },
        };

        self.upsert(&user).await
    }

    /// Removes the user with the specified email.
    pub async fn remove_by_email<E: AsRef<str>>(
        &self,
        user_email: E,
    ) -> anyhow::Result<Option<User>> {
        self.primary_db.remove_user_by_email(user_email).await
    }

    /// Retrieves data with the specified key for the user with the specified id.
    pub async fn get_data<R: DeserializeOwned>(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<Option<UserData<R>>> {
        self.primary_db.get_user_data(user_id, user_data_key).await
    }

    /// Stores data under the specified Key for the user with the specified id.
    pub async fn set_data(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let user_data_setter = UserDataSetter::new(user_id, &self.primary_db);
        match user_data_key.namespace {
            UserDataNamespace::Public(namespace) => match namespace {
                PublicUserDataNamespace::AutoResponders => {
                    Self::set_auto_responders_data(&user_data_setter, user_data).await
                }
                PublicUserDataNamespace::ContentSecurityPolicies => {
                    Self::set_content_security_policies_data(&user_data_setter, user_data).await
                }
                PublicUserDataNamespace::SelfSignedCertificates => {
                    Self::set_self_signed_certificates_data(&user_data_setter, user_data).await
                }
                PublicUserDataNamespace::UserSettings => {
                    Self::set_user_settings_data(&user_data_setter, user_data).await
                }
            },
            UserDataNamespace::Internal(_) => {
                user_data_setter.upsert(user_data_key, user_data).await
            }
        }
    }

    /// Generates a new account activation link for the specified user and sends to the user's email.
    pub async fn send_activation_link(&self, user: &User) -> anyhow::Result<()> {
        let mut bytes = [0u8; ACTIVATION_CODE_LENGTH_BYTES];
        OsRng.fill_bytes(&mut bytes);
        let activation_code: String = bytes.encode_hex();

        let timestamp = OffsetDateTime::now_utc();
        let namespace = InternalUserDataNamespace::AccountActivationToken;

        // Cleanup already expired activation codes.
        self.primary_db
            .cleanup_user_data(namespace, timestamp.sub(ACTIVATION_CODE_LIFESPAN))
            .await
            .with_context(|| "Failed to cleanup expired activation codes.")?;

        // Save newly created activation code.
        self.primary_db
            .upsert_user_data(
                user.id,
                namespace,
                UserData::new(&activation_code, timestamp),
            )
            .await
            .with_context(|| {
                format!(
                    "Cannot store activation code for the user (user ID: {:?})",
                    user.id
                )
            })?;

        let encoded_activation_link = format!(
            "{}activate?code={}&email={}",
            self.config.public_url.as_str(),
            urlencoding::encode(&activation_code),
            urlencoding::encode(&user.email)
        );
        self.emails.send(Email::new(
            &user.email,
            "Activate you Secutils.dev account",
            EmailBody::Html {
                content: format!(r#"
<!DOCTYPE html>
<html>
  <head>
    <title>Activate your Secutils.dev account</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
      body {{
        font-family: Arial, sans-serif;
        background-color: #f1f1f1;
        margin: 0;
        padding: 0;
      }}
      .container {{
        max-width: 600px;
        margin: 0 auto;
        background-color: #fff;
        padding: 20px;
        border-radius: 5px;
        box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
      }}
      h1 {{
        font-size: 24px;
        margin-top: 0;
      }}
      p {{
        font-size: 16px;
        line-height: 1.5;
        margin-bottom: 20px;
      }}
      .activate-link {{
        color: #fff;
        background-color: #2196F3;
        padding: 10px 20px;
        text-decoration: none;
        border-radius: 5px;
      }}
    </style>
  </head>
  <body>
    <div class="container">
      <h1>Activate your Secutils.dev account</h1>
      <p>Thanks for signing up! To activate your account, please click the link below:</p>
      <a class="activate-link" href="{encoded_activation_link}">Activate my account</a>
      <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>
      <p>{encoded_activation_link}</p>
      <p>If you have any trouble activating your account, please contact us at <a href = "mailto: contact@secutils.dev">contact@secutils.dev</a>.</p>
    </div>
  </body>
</html>"#),
                fallback: format!("To activate your Secutils.dev account, please click the following link: {encoded_activation_link}"),
            },
        ).with_timestamp(SystemTime::now()))
    }

    /// Generates credentials reset link for the specified user and sends to the user's email.
    pub async fn send_credentials_reset_link(&self, user: &User) -> anyhow::Result<()> {
        let mut bytes = [0u8; CREDENTIALS_RESET_CODE_LENGTH_BYTES];
        OsRng.fill_bytes(&mut bytes);
        let reset_code: String = bytes.encode_hex();

        let timestamp = OffsetDateTime::now_utc();
        let namespace = InternalUserDataNamespace::CredentialsResetToken;

        // Cleanup already expired codes.
        self.primary_db
            .cleanup_user_data(namespace, timestamp.sub(CREDENTIALS_RESET_CODE_LIFESPAN))
            .await
            .with_context(|| "Failed to cleanup expired credentials reset codes.")?;

        // Save newly created credentials reset code.
        self.primary_db
            .upsert_user_data(user.id, namespace, UserData::new(&reset_code, timestamp))
            .await
            .with_context(|| {
                format!(
                    "Cannot store credentials reset code for the user (user ID: {:?})",
                    user.id
                )
            })?;

        // For now we send email tailored for the password reset, but eventually we can allow user
        // to reset passkey as well.
        let encoded_reset_link = format!(
            "{}reset_credentials?code={}&email={}",
            self.config.public_url.as_str(),
            urlencoding::encode(&reset_code),
            urlencoding::encode(&user.email)
        );
        self.emails.send(Email::new(
            &user.email,
            "Reset password for your Secutils.dev account",
            EmailBody::Html {
                content: format!(r#"
<!DOCTYPE html>
<html>
  <head>
    <title>Reset password for your Secutils.dev account</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
      body {{
        font-family: Arial, sans-serif;
        background-color: #f1f1f1;
        margin: 0;
        padding: 0;
      }}
      .container {{
        max-width: 600px;
        margin: 0 auto;
        background-color: #fff;
        padding: 20px;
        border-radius: 5px;
        box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
      }}
      h1 {{
        font-size: 24px;
        margin-top: 0;
      }}
      p {{
        font-size: 16px;
        line-height: 1.5;
        margin-bottom: 20px;
      }}
      .reset-password-link {{
        color: #fff;
        background-color: #2196F3;
        padding: 10px 20px;
        text-decoration: none;
        border-radius: 5px;
      }}
    </style>
  </head>
  <body>
    <div class="container">
      <h1>Reset password for your Secutils.dev account</h1>
      <p>You recently requested to reset your password. To reset your password, please click the link below:</p>
      <a class="reset-password-link" href="{encoded_reset_link}">Reset your password</a>
      <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>
      <p>{encoded_reset_link}</p>
      <p>If you did not request to reset your password, please ignore this email and your password will not be changed.</p>
      <p>If you have any trouble resetting your password, please contact us at <a href = "mailto: contact@secutils.dev">contact@secutils.dev</a>.</p>
    </div>
  </body>
</html>"#),
                fallback: format!("To reset your Secutils.dev password, please click the following link: {encoded_reset_link}"),
            },
        ).with_timestamp(SystemTime::now()))
    }

    async fn set_auto_responders_data(
        user_data_setter: &UserDataSetter<'_>,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let auto_responders = serde_json::from_slice::<BTreeMap<String, Option<AutoResponder>>>(
            &serialized_user_data.value,
        )
        .with_context(|| "Cannot deserialize new autoresponders data".to_string())?;

        for (auto_responder_name, auto_responder) in auto_responders.iter() {
            match auto_responder {
                Some(auto_responder) if !auto_responder.is_valid() => {
                    bail!("Responder `{auto_responder_name}` is not valid: {auto_responder:?}");
                }
                Some(auto_responder) => {
                    log::debug!("Upserting `{auto_responder_name}` responder: {auto_responder:?}");
                }
                None => {
                    log::debug!("Removing `{auto_responder_name}` responder and its requests.");
                    user_data_setter
                        .remove((
                            PublicUserDataNamespace::AutoResponders,
                            auto_responder_name.as_str(),
                        ))
                        .await?;
                }
            }
        }

        DictionaryDataUserDataSetter::upsert(
            user_data_setter,
            PublicUserDataNamespace::AutoResponders,
            UserData::new(auto_responders, serialized_user_data.timestamp),
        )
        .await
    }

    async fn set_user_settings_data(
        user_data_setter: &UserDataSetter<'_>,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let user_settings =
            serde_json::from_slice::<UserSettingsSetter>(&serialized_user_data.value)
                .with_context(|| "Cannot deserialize new user settings data".to_string())?;
        if !user_settings.is_valid() {
            bail!("User settings are not valid: {:?}", user_settings);
        }
        DictionaryDataUserDataSetter::upsert(
            user_data_setter,
            PublicUserDataNamespace::UserSettings,
            UserData::new(user_settings.into_inner(), serialized_user_data.timestamp),
        )
        .await
    }

    async fn set_content_security_policies_data(
        user_data_setter: &UserDataSetter<'_>,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        DictionaryDataUserDataSetter::upsert(
            user_data_setter,
            PublicUserDataNamespace::ContentSecurityPolicies,
            UserData::new(
                serde_json::from_slice::<BTreeMap<String, Option<ContentSecurityPolicy>>>(
                    &serialized_user_data.value,
                )
                .with_context(|| {
                    "Cannot deserialize new content security policies data".to_string()
                })?,
                serialized_user_data.timestamp,
            ),
        )
        .await
    }

    async fn set_self_signed_certificates_data(
        user_data_setter: &UserDataSetter<'_>,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        DictionaryDataUserDataSetter::upsert(
            user_data_setter,
            PublicUserDataNamespace::SelfSignedCertificates,
            UserData::new(
                serde_json::from_slice::<BTreeMap<String, Option<SelfSignedCertificate>>>(
                    &serialized_user_data.value,
                )
                .with_context(|| {
                    "Cannot deserialize new self-signed certificates data".to_string()
                })?,
                serialized_user_data.timestamp,
            ),
        )
        .await
    }

    /// Generates a random user handle (8 bytes).
    async fn generate_user_handle(&self) -> anyhow::Result<String> {
        let mut bytes = [0u8; USER_HANDLE_LENGTH_BYTES];
        loop {
            OsRng.fill_bytes(&mut bytes);
            let handle = bytes.encode_hex::<String>();
            if self.primary_db.get_user_by_handle(&handle).await?.is_none() {
                return Ok(handle);
            }
        }
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
                    .primary_db
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
                    .webauthn
                    .finish_passkey_registration(
                        &serde_json::from_value::<RegisterPublicKeyCredential>(
                            serialized_public_key,
                        )?,
                        &registration_state,
                    )
                    .map(StoredCredentials::from_passkey)?;

                // Clear WebAuthn session state since we no longer need it.
                self.primary_db
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
}

#[cfg(test)]
mod tests {
    use crate::{
        api::{EmailsApi, UsersApi},
        authentication::{create_webauthn, StoredCredentials},
        config::Config,
        datastore::PrimaryDb,
        tests::{mock_db, MockUserBuilder},
        users::{PublicUserDataNamespace, User, UserData, UserId},
        utils::{AutoResponder, AutoResponderMethod, AutoResponderRequest},
    };
    use std::{borrow::Cow, collections::BTreeMap};
    use time::OffsetDateTime;
    use url::Url;

    fn create_mock_user() -> User {
        MockUserBuilder::new(
            UserId(1),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials::try_from_password("password").unwrap(),
            OffsetDateTime::now_utc(),
        )
        .build()
    }

    fn create_mock_config() -> Config {
        Config {
            version: "1.0.0".to_string(),
            http_port: 1234,
            public_url: Url::parse("http://localhost:1234").unwrap(),
            smtp: None,
        }
    }

    async fn initialize_mock_db(user: &User) -> anyhow::Result<PrimaryDb> {
        let db = mock_db().await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn can_update_auto_responders() -> anyhow::Result<()> {
        let mock_config = create_mock_config();
        let mock_user = create_mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_webauthn = create_webauthn(&mock_config)?;
        let api = UsersApi::new(
            &mock_config,
            &mock_webauthn,
            &mock_db,
            EmailsApi::new(&mock_config),
        );

        let auto_responder_one = AutoResponder {
            name: "name-one".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        let auto_responder_two = AutoResponder {
            name: "name-two".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        let auto_responder_two_new = AutoResponder {
            name: "name-two".to_string(),
            method: AutoResponderMethod::Post,
            requests_to_track: 10,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };

        // Insert auto responders data.
        api.set_data(
            mock_user.id,
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                serde_json::to_vec(
                    &[
                        (&auto_responder_one.name, auto_responder_one.clone()),
                        (&auto_responder_two.name, auto_responder_two.clone()),
                    ]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                )?,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;

        let request_one = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };
        let request_two = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.2".parse()?),
            method: Cow::Borrowed("POST"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };

        // Insert auto responder requests.
        mock_db
            .upsert_user_data(
                mock_user.id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder_one.name.as_str(),
                ),
                UserData::new(
                    vec![request_one.clone()],
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                ),
            )
            .await?;
        mock_db
            .upsert_user_data(
                mock_user.id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder_two.name.as_str(),
                ),
                UserData::new(
                    vec![request_two.clone()],
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                ),
            )
            .await?;

        // Verify that requests were inserted.
        assert_eq!(
            mock_db
                .get_user_data(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_one.name.as_str(),
                    )
                )
                .await?,
            Some(UserData::new(
                vec![request_one],
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );
        assert_eq!(
            mock_db
                .get_user_data(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_two.name.as_str(),
                    ),
                )
                .await?,
            Some(UserData::new(
                vec![request_two.clone()],
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Remove one auto responder and update another.
        api.set_data(
            mock_user.id,
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                serde_json::to_vec(
                    &[
                        (&auto_responder_one.name, None),
                        (
                            &auto_responder_two.name,
                            Some(auto_responder_two_new.clone()),
                        ),
                    ]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                )?,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;

        // Verify that auto responders were correctly updated.
        assert_eq!(
            api.get_data(mock_user.id, PublicUserDataNamespace::AutoResponders)
                .await?,
            Some(UserData::new(
                [(auto_responder_two.name, auto_responder_two_new.clone())]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Verify that requests were updated.
        assert_eq!(
            mock_db
                .get_user_data::<Vec<AutoResponderRequest>>(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_one.name.as_str(),
                    ),
                )
                .await?,
            None
        );
        assert_eq!(
            mock_db
                .get_user_data(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_two_new.name.as_str(),
                    ),
                )
                .await?,
            Some(UserData::new(
                vec![request_two],
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        Ok(())
    }
}
