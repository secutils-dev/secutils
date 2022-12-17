use crate::{
    api::{Email, EmailBody, EmailsApi},
    config::Config,
    datastore::{UsersIndex, UsersSearchFilter},
    users::{BuiltinUser, User},
};
use anyhow::{anyhow, bail, Context};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use hex::ToHex;
use rand_core::{OsRng, RngCore};
use std::{borrow::Cow, collections::HashSet, time::SystemTime};
use time::OffsetDateTime;

const USER_HANDLE_LENGTH_BYTES: usize = 8;
const ACTIVATION_CODE_LENGTH_BYTES: usize = 32;

pub struct UsersApi<'a> {
    emails: EmailsApi<&'a Config>,
    users_index: Cow<'a, UsersIndex>,
}

impl<'a> UsersApi<'a> {
    /// Creates Users API.
    pub fn new(users_index: &'a UsersIndex, emails: EmailsApi<&'a Config>) -> Self {
        Self {
            emails,
            users_index: Cow::Borrowed(users_index),
        }
    }

    /// Signs up a user with the specified email and password. If the user with such email is
    /// already registered, this method throws.
    /// NOTE: User isn't required to activate profile right away and can use application without
    /// activation. After signup, we'll send email with the activation code, and will re-send it
    /// after 7 days, then after 14 days, and after 30 days we'll terminate account with a large
    /// warning in the application. Users will be able to request another activation link from their
    /// profile page.
    pub fn signup<E: Into<String>, P: AsRef<str>>(
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
        if let Some(user) = self.users_index.get(&user_email)? {
            bail!(
                "Cannot signup user: a user {} with the same email already exists.",
                user.handle
            );
        }

        let activation_code = Self::generate_activation_code();
        let user = User {
            email: user_email,
            handle: self.generate_user_handle()?,
            password_hash: Self::generate_user_password_hash(user_password)?,
            created: OffsetDateTime::now_utc(),
            roles: HashSet::with_capacity(0),
            profile: None,
            activation_code: Some(activation_code.clone()),
        };

        // TODO: Activation code should be URL encoded.
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
        <p style="font-family: Arial, Helvetica, sans-serif;">Activation code: <a href="https://secutils.dev/activation/{}">{}</a>!</p>
    </div>
</body>
</html>"#,
                    activation_code,
                    activation_code
                ),
                fallback: format!("Activation code: {}", activation_code),
            },
        ).with_timestamp(SystemTime::now()))?;

        self.users_index.upsert(&user).with_context(|| {
            format!("Cannot signup user: failed to upsert user {}", user.handle)
        })?;

        Ok(user)
    }

    /// Authenticates user with the specified email and password.
    pub fn authenticate<EP: AsRef<str>>(
        &self,
        user_email: EP,
        user_password: EP,
    ) -> anyhow::Result<User> {
        let user = if let Some(user) = self.users_index.get(user_email.as_ref())? {
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
    pub fn activate<A: AsRef<str>>(&self, activation_code: A) -> anyhow::Result<User> {
        let mut users = self
            .users_index
            .search(UsersSearchFilter::default().with_activation_code(activation_code.as_ref()))?;
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

        self.users_index
            .upsert(&user_to_activate)
            .with_context(|| {
                format!(
                    "Cannot activate user: failed to store activated user {}",
                    user_to_activate.handle
                )
            })?;

        Ok(user_to_activate)
    }

    /// Retrieves the user using the specified email.
    pub fn get<E: AsRef<str>>(&self, user_email: E) -> anyhow::Result<Option<User>> {
        self.users_index.get(user_email)
    }

    /// Retrieves the user using the specified handle.
    pub fn get_by_handle<E: AsRef<str>>(&self, user_handle: E) -> anyhow::Result<Option<User>> {
        self.users_index.get_by_handle(user_handle)
    }

    /// Inserts or updates user in the `Users` store.
    pub fn upsert<U: AsRef<User>>(&self, user: U) -> anyhow::Result<()> {
        self.users_index.upsert(user)
    }

    /// Inserts or updates user in the `Users` store using `BuiltinUser`.
    pub fn upsert_builtin(&self, builtin_user: BuiltinUser) -> anyhow::Result<()> {
        let user = match self.get(&builtin_user.email)? {
            Some(user) => User {
                email: user.email,
                handle: user.handle,
                created: user.created,
                profile: user.profile,
                password_hash: builtin_user.password_hash,
                roles: builtin_user.roles,
                activation_code: None,
            },
            None => User {
                email: builtin_user.email,
                handle: self.generate_user_handle()?,
                password_hash: builtin_user.password_hash,
                created: OffsetDateTime::now_utc(),
                roles: builtin_user.roles,
                profile: None,
                activation_code: None,
            },
        };

        self.upsert(user)
    }

    /// Removes the user with the specified email.
    pub fn remove<E: AsRef<str>>(&self, user_email: E) -> anyhow::Result<Option<User>> {
        self.users_index.remove(user_email)
    }

    /// Generates random user handle (8 bytes).
    fn generate_user_handle(&self) -> anyhow::Result<String> {
        let mut bytes = [0u8; USER_HANDLE_LENGTH_BYTES];
        loop {
            OsRng.fill_bytes(&mut bytes);
            let handle = bytes.encode_hex::<String>();
            if self.users_index.get_by_handle(&handle)?.is_none() {
                return Ok(handle);
            }
        }
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

    fn generate_activation_code() -> String {
        let mut bytes = [0u8; ACTIVATION_CODE_LENGTH_BYTES];
        OsRng.fill_bytes(&mut bytes);
        bytes.encode_hex()
    }
}
