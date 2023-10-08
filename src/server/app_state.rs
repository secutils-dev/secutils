use crate::{
    api::Api,
    config::Config,
    network::{DnsResolver, EmailTransport, TokioDnsResolver},
    security::Security,
    server::{Status, StatusLevel},
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

#[cfg(test)]
pub mod tests {
    use crate::{
        api::Api,
        network::{Network, TokioDnsResolver},
        security::{create_webauthn, Security},
        server::AppState,
        templates::create_templates,
        tests::{mock_config, mock_db, mock_search_index},
    };
    use lettre::{AsyncSmtpTransport, Tokio1Executor};
    use std::sync::Arc;

    pub async fn mock_app_state() -> anyhow::Result<AppState> {
        let api = Arc::new(Api::new(
            mock_config()?,
            mock_db().await?,
            mock_search_index()?,
            // We should use a real network implementation in tests that rely on `AppState` being
            // extracted from `HttpRequest`, as types should match for the extraction to work.
            Network::new(
                TokioDnsResolver::create(),
                AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost(),
            ),
            create_templates()?,
        ));

        Ok(AppState::new(
            api.config.clone(),
            Security::new(api.clone(), create_webauthn(&api.config)?),
            api,
        ))
    }
}
