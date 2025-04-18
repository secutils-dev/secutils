use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        DictionaryDataUserDataSetter, SharedResource, User, UserData, UserDataKey,
        UserDataNamespace, UserId, UserSettingsSetter, UserShare, UserShareId,
    },
};
use anyhow::{Context, bail};
use serde::Deserialize;

pub mod errors;
pub mod user_data_setters;

pub struct UsersApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> UsersApi<'a, DR, ET> {
    /// Creates Users API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Retrieves the user by the specified ID.
    pub async fn get(&self, id: UserId) -> anyhow::Result<Option<User>> {
        self.api.db.get_user(id).await
    }

    /// Retrieves the user using the specified email.
    pub async fn get_by_email<E: AsRef<str>>(&self, user_email: E) -> anyhow::Result<Option<User>> {
        self.api.db.get_user_by_email(user_email).await
    }

    /// Retrieves the user using the specified handle.
    pub async fn get_by_handle<E: AsRef<str>>(
        &self,
        user_handle: E,
    ) -> anyhow::Result<Option<User>> {
        self.api.db.get_user_by_handle(user_handle).await
    }

    /// Retrieves data with the specified key for the user with the specified id.
    pub async fn get_data<R: for<'de> Deserialize<'de>>(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<Option<UserData<R>>> {
        self.api.db.get_user_data(user_id, user_data_key).await
    }

    /// Stores user data under the specified key.
    pub async fn set_data(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        match user_data_key.namespace {
            UserDataNamespace::UserSettings => self.set_user_settings_data(user_data).await,
        }
    }

    /// Retrieves the user share by the specified ID.
    pub async fn get_user_share(&self, id: UserShareId) -> anyhow::Result<Option<UserShare>> {
        self.api.db.get_user_share(id).await
    }

    /// Retrieves the user share by the specified user ID and resource.
    pub async fn get_user_share_by_resource(
        &self,
        user_id: UserId,
        resource: &SharedResource,
    ) -> anyhow::Result<Option<UserShare>> {
        self.api
            .db
            .get_user_share_by_resource(user_id, resource)
            .await
    }

    /// Inserts user share into the database.
    pub async fn insert_user_share(&self, user_share: &UserShare) -> anyhow::Result<()> {
        self.api.db.insert_user_share(user_share).await
    }

    /// Removes user share with the specified ID from the database.
    pub async fn remove_user_share(&self, id: UserShareId) -> anyhow::Result<Option<UserShare>> {
        self.api.db.remove_user_share(id).await
    }

    async fn set_user_settings_data(
        &self,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let user_settings =
            serde_json::from_slice::<UserSettingsSetter>(&serialized_user_data.value)
                .with_context(|| "Cannot deserialize new user settings data".to_string())?;
        if !user_settings.is_valid() {
            bail!("User settings are not valid: {:?}", user_settings);
        }
        DictionaryDataUserDataSetter::upsert(
            &self.api.db,
            UserDataNamespace::UserSettings,
            UserData::new(
                serialized_user_data.user_id,
                user_settings.into_inner(),
                serialized_user_data.timestamp,
            ),
        )
        .await
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with users.
    pub fn users(&self) -> UsersApi<DR, ET> {
        UsersApi::new(self)
    }
}
