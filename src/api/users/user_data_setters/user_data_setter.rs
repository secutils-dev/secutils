use crate::{datastore::PrimaryDb, users::UserDataType};
use serde::{de::DeserializeOwned, Serialize};

/// Abstracts away database and methods bound to a specific user from the data setters.
pub struct UserDataSetter<'a> {
    user_email: &'a str,
    primary_db: &'a PrimaryDb,
}

impl<'a> UserDataSetter<'a> {
    /// Creates a data setter bound to a user with the specified email.
    pub fn new(user_email: &'a str, primary_db: &'a PrimaryDb) -> Self {
        Self {
            user_email,
            primary_db,
        }
    }

    /// Gets user data of the specific data type.
    pub async fn get<R: DeserializeOwned>(
        &self,
        data_type: UserDataType,
    ) -> anyhow::Result<Option<R>> {
        self.primary_db
            .get_user_data(self.user_email, data_type)
            .await
    }

    /// Inserts new or updates existing user data of the specified type.
    pub async fn upsert<R: Serialize>(
        &self,
        data_type: UserDataType,
        data_value: R,
    ) -> anyhow::Result<()> {
        self.primary_db
            .upsert_user_data(self.user_email, data_type, data_value)
            .await
    }

    /// Removes existing user data of the specified type.
    pub async fn remove(&self, data_type: UserDataType) -> anyhow::Result<()> {
        self.primary_db
            .remove_user_data(self.user_email, data_type)
            .await
    }
}
