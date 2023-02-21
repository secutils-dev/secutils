use crate::{
    api::Api,
    config::Config,
    server::status::{Status, StatusLevel},
    users::{User, UserRole},
};
use actix_web::{error::ErrorForbidden, Error};
use anyhow::anyhow;
use std::sync::RwLock;

pub struct AppState {
    pub config: Config,
    pub status: RwLock<Status>,
    pub api: Api,
}

impl AppState {
    pub fn new(config: Config, api: Api) -> Self {
        Self {
            config,

            status: RwLock::new(Status {
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
