use crate::{
    api::Api,
    config::Config,
    network::{DnsResolver, Network, TokioDnsResolver},
    scheduler::Scheduler,
    server::status::{Status, StatusLevel},
    users::{User, UserRole},
};
use actix_web::{error::ErrorForbidden, Error};
use anyhow::anyhow;
use std::sync::RwLock;

pub struct AppState<DR: DnsResolver = TokioDnsResolver> {
    pub config: Config,
    pub status: RwLock<Status>,
    pub api: Api,
    pub network: Network<DR>,
    pub scheduler: Scheduler,
}

impl<DR: DnsResolver> AppState<DR> {
    pub fn new(config: Config, api: Api, network: Network<DR>, scheduler: Scheduler) -> Self {
        let version = config.version.to_string();
        Self {
            config,
            status: RwLock::new(Status {
                version,
                level: StatusLevel::Available,
            }),
            api,
            network,
            scheduler,
        }
    }

    pub fn ensure_admin(&self, user: &User) -> Result<(), Error> {
        if !user.roles.contains(UserRole::ADMIN_ROLE) {
            return Err(ErrorForbidden(anyhow!("Forbidden")));
        }

        Ok(())
    }
}
