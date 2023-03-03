use crate::{
    datastore::PrimaryDb,
    users::{UserData, UserDataKey, UserId},
};
use serde::{de::DeserializeOwned, Serialize};

/// Abstracts away database and methods bound to a specific user from the data setters.
pub struct UserDataSetter<'a> {
    user_id: UserId,
    primary_db: &'a PrimaryDb,
}

impl<'a> UserDataSetter<'a> {
    /// Creates a data setter bound to a user with the specified id.
    pub fn new(user_id: UserId, primary_db: &'a PrimaryDb) -> Self {
        Self {
            user_id,
            primary_db,
        }
    }

    /// Gets user data by the specific data key.
    pub async fn get<R: DeserializeOwned>(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<Option<UserData<R>>> {
        self.primary_db
            .get_user_data(self.user_id, user_data_key)
            .await
    }

    /// Inserts new or updates existing user data by the specified data key.
    pub async fn upsert<R: Serialize>(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<R>,
    ) -> anyhow::Result<()> {
        self.primary_db
            .upsert_user_data(self.user_id, user_data_key, user_data)
            .await
    }

    /// Removes existing user data by the specified data key.
    pub async fn remove(&self, user_data_key: impl Into<UserDataKey<'_>>) -> anyhow::Result<()> {
        self.primary_db
            .remove_user_data(self.user_id, user_data_key)
            .await
    }
}
