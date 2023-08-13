mod emails;
mod users;
mod utils;

pub use self::{
    emails::{Email, EmailBody, EmailsApi},
    users::{UserSignupError, UsersApi},
    utils::UtilsApi,
};

use crate::{
    database::Database,
    network::{DnsResolver, Network},
    search::SearchIndex,
    Config,
};

pub(crate) use self::users::DictionaryDataUserDataSetter;

pub struct Api<DR: DnsResolver> {
    pub db: Database,
    pub search_index: SearchIndex,
    pub config: Config,
    pub network: Network<DR>,
}

impl<DR: DnsResolver> Api<DR> {
    /// Instantiates APIs collection with the specified config and datastore.
    pub fn new(
        config: Config,
        database: Database,
        search_index: SearchIndex,
        network: Network<DR>,
    ) -> Self {
        Self {
            config,
            db: database,
            search_index,
            network,
        }
    }

    /// Returns an API to work with users.
    pub fn users(&self) -> UsersApi {
        UsersApi::new(&self.db)
    }

    /// Returns an API to send emails.
    pub fn emails(&self) -> EmailsApi<&Config> {
        EmailsApi::new(&self.config)
    }

    /// Returns an API to retrieve available utils.
    pub fn utils(&self) -> UtilsApi {
        UtilsApi::new(&self.db)
    }
}

impl<DR: DnsResolver> AsRef<Api<DR>> for Api<DR> {
    fn as_ref(&self) -> &Self {
        self
    }
}
