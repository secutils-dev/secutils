use crate::{
    api::{
        users::{AutoRespondersUserDataSetter, RootCertificatesUserDataSetter, UserDataSetter},
        Email, EmailBody, EmailsApi,
    },
    config::Config,
    datastore::PrimaryDb,
    users::{BuiltinUser, User, UserDataType, UserId},
};
use anyhow::{anyhow, bail, Context};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use hex::ToHex;
use rand_core::{OsRng, RngCore};
use serde::de::DeserializeOwned;
use std::{borrow::Cow, collections::HashSet, time::SystemTime};
use time::OffsetDateTime;

const USER_HANDLE_LENGTH_BYTES: usize = 8;
const ACTIVATION_CODE_LENGTH_BYTES: usize = 32;

pub struct UsersApi<'a> {
    emails: EmailsApi<&'a Config>,
    primary_db: Cow<'a, PrimaryDb>,
}

impl<'a> UsersApi<'a> {
    /// Creates Users API.
    pub fn new(primary_db: &'a PrimaryDb, emails: EmailsApi<&'a Config>) -> Self {
        Self {
            emails,
            primary_db: Cow::Borrowed(primary_db),
        }
    }

    /// Signs up a user with the specified email and password. If the user with such email is
    /// already registered, this method throws.
    /// NOTE: User isn't required to activate profile right away and can use application without
    /// activation. After signup, we'll send email with the activation code, and will re-send it
    /// after 7 days, then after 14 days, and after 30 days we'll terminate account with a large
    /// warning in the application. Users will be able to request another activation link from their
    /// profile page.
    pub async fn signup<E: Into<String>, P: AsRef<str>>(
        &self,
        user_email: E,
        user_password: P,
    ) -> anyhow::Result<User> {
        let user_email = user_email.into();
        if user_email.is_empty() || !user_email.contains('@') {
            bail!("Cannot signup user: invalid user email `{}`.", user_email);
        }

        if user_password.as_ref().is_empty() {
            bail!("Cannot signup user: empty user password.");
        }

        // Prevent multiple signup requests from the same user.
        if let Some(user) = self.primary_db.get_user_by_email(&user_email).await? {
            bail!(
                "Cannot signup user: a user {} with the same email already exists.",
                user.handle
            );
        }

        let activation_code = Self::generate_activation_code();
        let user = User {
            id: UserId::empty(),
            email: user_email,
            handle: self.generate_user_handle().await?,
            password_hash: Self::generate_user_password_hash(user_password)?,
            created: OffsetDateTime::now_utc(),
            roles: HashSet::with_capacity(0),
            activation_code: Some(activation_code.clone()),
        };

        // TODO: Activation code should be URL encoded.
        // TODO: Don't hardcode `secutils.dev` in activation email - make it configurable
        self.emails.send(Email::new(
            &user.email,
            "Activate you Secutils.dev account",
            EmailBody::Html {
                content: format!(
                    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Activate your Secutils.dev account</title>
</head>
<body>
    <div style="display: flex; flex-direction: column; align-items: center;">
        <p style="font-family: Arial, Helvetica, sans-serif;">Activation code: <a href="https://secutils.dev/activation/{activation_code}">{activation_code}</a>!</p>
    </div>
</body>
</html>"#),
                fallback: format!("Activation code: {activation_code}"),
            },
        ).with_timestamp(SystemTime::now()))?;

        let user_id = self.primary_db.upsert_user(&user).await.with_context(|| {
            format!("Cannot signup user: failed to upsert user {}", user.handle)
        })?;

        Ok(User {
            id: user_id,
            ..user
        })
    }

    /// Authenticates user with the specified email and password.
    pub async fn authenticate<EP: AsRef<str>>(
        &self,
        user_email: EP,
        user_password: EP,
    ) -> anyhow::Result<User> {
        let user = if let Some(user) = self
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

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|err| anyhow!("Failed to parse a password hash: {}", err))?;
        Argon2::default()
            .verify_password(user_password.as_ref().as_bytes(), &parsed_hash)
            .map_err(|err| {
                anyhow!(
                    "Cannot authenticate user: failed to validate password for user {} due to {}",
                    user.handle,
                    err
                )
            })?;

        Ok(user)
    }

    /// Activates user using the specified activation code.
    pub async fn activate<A: AsRef<str>>(&self, activation_code: A) -> anyhow::Result<User> {
        let mut users = self
            .primary_db
            .get_users_by_activation_code(activation_code.as_ref())
            .await?;
        if users.is_empty() {
            bail!(
                "Cannot activate user: cannot find user to activate for the code {}.",
                activation_code.as_ref()
            );
        }

        if users.len() > 1 {
            bail!(
                "Cannot activate user: there are multiple users to activate with the same code {}.",
                activation_code.as_ref()
            );
        }

        let mut user_to_activate = users.remove(0);
        user_to_activate.activation_code.take();

        self.primary_db
            .upsert_user(&user_to_activate)
            .await
            .with_context(|| {
                format!(
                    "Cannot activate user: failed to store activated user {}",
                    user_to_activate.handle
                )
            })?;

        Ok(user_to_activate)
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
                password_hash: builtin_user.password_hash,
                roles: builtin_user.roles,
                activation_code: None,
            },
            None => User {
                id: UserId::empty(),
                email: builtin_user.email,
                handle: self.generate_user_handle().await?,
                password_hash: builtin_user.password_hash,
                created: OffsetDateTime::now_utc(),
                roles: builtin_user.roles,
                activation_code: None,
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

    /// Generates user password hash.
    pub fn generate_user_password_hash<P: AsRef<str>>(user_password: P) -> anyhow::Result<String> {
        Argon2::default()
            .hash_password(
                user_password.as_ref().as_bytes(),
                &SaltString::generate(&mut OsRng),
            )
            .map(|hash| hash.to_string())
            .map_err(|err| anyhow!("Failed to generate a password hash: {}", err))
    }

    /// Retrieves data of the specified type for the user with the specified id.
    pub async fn get_data<R: DeserializeOwned>(
        &self,
        user_id: UserId,
        data_type: UserDataType,
    ) -> anyhow::Result<Option<R>> {
        self.primary_db.get_user_data(user_id, data_type).await
    }

    /// Sets user data of the specified type for the user with the specified id.
    pub async fn set_data(
        &self,
        user_id: UserId,
        data_type: UserDataType,
        serialized_data_value: Vec<u8>,
    ) -> anyhow::Result<()> {
        let user_data_setter = UserDataSetter::new(user_id, &self.primary_db);
        match data_type {
            UserDataType::AutoResponders => {
                AutoRespondersUserDataSetter::upsert(&user_data_setter, serialized_data_value).await
            }
            UserDataType::RootCertificates => {
                RootCertificatesUserDataSetter::upsert(&user_data_setter, serialized_data_value)
                    .await
            }
        }
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

    fn generate_activation_code() -> String {
        let mut bytes = [0u8; ACTIVATION_CODE_LENGTH_BYTES];
        OsRng.fill_bytes(&mut bytes);
        bytes.encode_hex()
    }
}
