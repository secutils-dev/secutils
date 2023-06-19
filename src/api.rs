mod emails;
mod search;
mod users;
mod utils;

pub use self::{
    emails::{Email, EmailBody, EmailsApi},
    search::SearchApi,
    users::{UserSignupError, UsersApi},
    utils::UtilsApi,
};
use webauthn_rs::Webauthn;

use crate::{datastore::Datastore, Config};

pub(crate) use self::users::{DictionaryDataUserDataSetter, UserDataSetter};

pub struct Api {
    pub datastore: Datastore,
    pub config: Config,
    pub webauthn: Webauthn,
}

impl Api {
    /// Instantiates APIs collection with the specified config, datastore and other parameters.
    pub fn new(config: Config, datastore: Datastore, webauthn: Webauthn) -> Self {
        Self {
            config,
            datastore,
            webauthn,
        }
    }

    /// Returns an API to work with users.
    pub fn users(&self) -> UsersApi {
        UsersApi::new(
            &self.config,
            &self.webauthn,
            &self.datastore.primary_db,
            self.emails(),
        )
    }

    /// Returns an API to send emails.
    pub fn emails(&self) -> EmailsApi<&Config> {
        EmailsApi::new(&self.config)
    }

    /// Returns an API to perform application-wide search.
    pub fn search(&self) -> SearchApi {
        SearchApi::new(&self.datastore.search_index)
    }

    /// Returns an API to retrieve available utils.
    pub fn utils(&self) -> UtilsApi {
        UtilsApi::new(&self.datastore.primary_db)
    }
}

impl AsRef<Api> for Api {
    fn as_ref(&self) -> &Self {
        self
    }
}
