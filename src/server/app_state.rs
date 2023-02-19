use crate::{
    api::Api,
    config::Config,
    server::status::{Status, StatusLevel},
    users::{User, UserRole},
};
use actix_web::{error::ErrorForbidden, Error};
use anyhow::anyhow;
use std::sync::RwLock;
use webauthn_rs::Webauthn;

pub struct AppState {
    pub config: Config,
    pub webauthn: Webauthn,
    pub status: RwLock<Status>,
    pub api: Api,
}

impl AppState {
    pub fn new(config: Config, webauthn: Webauthn, api: Api) -> Self {
        Self {
            config,
            webauthn,

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
