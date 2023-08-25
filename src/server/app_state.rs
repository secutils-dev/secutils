use crate::{
    api::Api,
    config::Config,
    network::{DnsResolver, EmailTransport, TokioDnsResolver},
    security::Security,
    server::status::{Status, StatusLevel},
    users::{User, UserRole},
};
use actix_web::{error::ErrorForbidden, Error};
use anyhow::anyhow;
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use std::sync::{Arc, RwLock};

pub struct AppState<
    DR: DnsResolver = TokioDnsResolver,
    ET: EmailTransport = AsyncSmtpTransport<Tokio1Executor>,
> {
    pub config: Config,
    pub status: RwLock<Status>,
    pub api: Arc<Api<DR, ET>>,
    pub security: Security<DR, ET>,
}

impl<DR: DnsResolver, ET: EmailTransport> AppState<DR, ET> {
    pub fn new(config: Config, security: Security<DR, ET>, api: Arc<Api<DR, ET>>) -> Self {
        let version = config.version.to_string();
        Self {
            config,
            security,
            status: RwLock::new(Status {
                version,
                level: StatusLevel::Available,
            }),
            api,
        }
    }

    pub fn ensure_admin(&self, user: &User) -> Result<(), Error> {
        if !user.roles.contains(UserRole::ADMIN_ROLE) {
            return Err(ErrorForbidden(anyhow!("Forbidden")));
        }

        Ok(())
    }
}
