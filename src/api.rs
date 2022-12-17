mod emails;
mod users;

pub use self::{
    emails::{Email, EmailBody, EmailsApi},
    users::UsersApi,
};

use crate::{datastore::Datastore, Config};

#[derive(Clone)]
pub struct Api {
    datastore: Datastore,
    config: Config,
}

impl Api {
    /// Instantiates APIs collection with the specified config and datastore.
    pub fn new(config: Config, datastore: Datastore) -> Self {
        Self { config, datastore }
    }

    /// Returns an API to work with users.
    pub fn users(&self) -> UsersApi {
        UsersApi::new(&self.datastore.users, self.emails())
    }

    pub fn emails(&self) -> EmailsApi<&Config> {
        EmailsApi::new(&self.config)
    }
}

impl AsRef<Api> for Api {
    fn as_ref(&self) -> &Self {
        self
    }
}
