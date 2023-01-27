mod emails;
mod search;
mod users;

pub use self::{
    emails::{Email, EmailBody, EmailsApi},
    search::SearchApi,
    users::UsersApi,
};

use crate::{datastore::Datastore, Config};

#[derive(Clone)]
pub struct Api {
    pub datastore: Datastore,
    pub config: Config,
}

impl Api {
    /// Instantiates APIs collection with the specified config and datastore.
    pub fn new(config: Config, datastore: Datastore) -> Self {
        Self { config, datastore }
    }

    /// Returns an API to work with users.
    pub fn users(&self) -> UsersApi {
        UsersApi::new(&self.datastore.primary_db, self.emails())
    }

    /// Returns an API to send emails.
    pub fn emails(&self) -> EmailsApi<&Config> {
        EmailsApi::new(&self.config)
    }

    /// Returns an API to perform application-wide search.
    pub fn search(&self) -> SearchApi {
        SearchApi::new(&self.datastore.search_index)
    }
}

impl AsRef<Api> for Api {
    fn as_ref(&self) -> &Self {
        self
    }
}
