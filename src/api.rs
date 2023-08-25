use crate::{
    database::Database,
    network::{DnsResolver, EmailTransport, Network},
    search::SearchIndex,
    Config,
};

pub struct Api<DR: DnsResolver, ET: EmailTransport> {
    pub db: Database,
    pub search_index: SearchIndex,
    pub config: Config,
    pub network: Network<DR, ET>,
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Instantiates APIs collection with the specified config and datastore.
    pub fn new(
        config: Config,
        database: Database,
        search_index: SearchIndex,
        network: Network<DR, ET>,
    ) -> Self {
        Self {
            config,
            db: database,
            search_index,
            network,
        }
    }
}

impl<DR: DnsResolver, ET: EmailTransport> AsRef<Api<DR, ET>> for Api<DR, ET> {
    fn as_ref(&self) -> &Self {
        self
    }
}
