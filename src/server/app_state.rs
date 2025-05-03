use crate::{
    api::Api,
    config::Config,
    network::{DnsResolver, EmailTransport, TokioDnsResolver},
    server::{Status, StatusLevel},
};
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use std::sync::{Arc, RwLock};

pub struct AppState<
    DR: DnsResolver = TokioDnsResolver,
    ET: EmailTransport = AsyncSmtpTransport<Tokio1Executor>,
> {
    pub config: Config,
    pub status: RwLock<Status>,
    pub api: Arc<Api<DR, ET>>,
}

impl<DR: DnsResolver, ET: EmailTransport> AppState<DR, ET> {
    pub fn new(config: Config, api: Arc<Api<DR, ET>>) -> Self {
        Self {
            config,
            status: RwLock::new(Status {
                version: env!("CARGO_PKG_VERSION").to_string(),
                level: StatusLevel::Available,
            }),
            api,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        api::Api,
        config::Config,
        database::Database,
        network::{Network, TokioDnsResolver},
        server::AppState,
        templates::create_templates,
        tests::{mock_config, mock_search_index},
    };
    use lettre::{AsyncSmtpTransport, Tokio1Executor};
    use reqwest::Client;
    use reqwest_middleware::ClientBuilder;
    use sqlx::PgPool;
    use std::sync::Arc;

    pub async fn mock_app_state(pool: PgPool) -> anyhow::Result<AppState> {
        mock_app_state_with_config(pool, mock_config()?).await
    }

    pub async fn mock_app_state_with_config(
        pool: PgPool,
        config: Config,
    ) -> anyhow::Result<AppState> {
        let api = Arc::new(Api::new(
            config,
            Database::create(pool).await?,
            mock_search_index()?,
            // We should use a real network implementation in tests that rely on `AppState` being
            // extracted from `HttpRequest`, as types should match for the extraction to work.
            Network::new(
                TokioDnsResolver::create(),
                AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost(),
                ClientBuilder::new(Client::builder().build()?).build(),
            ),
            create_templates()?,
        ));

        Ok(AppState::new(api.config.clone(), api))
    }
}
