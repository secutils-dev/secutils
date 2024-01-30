use crate::{
    api::Api,
    config::Config,
    network::{DnsResolver, EmailTransport, TokioDnsResolver},
    server::{Status, StatusLevel},
    users::User,
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
}

impl<DR: DnsResolver, ET: EmailTransport> AppState<DR, ET> {
    pub fn new(config: Config, api: Arc<Api<DR, ET>>) -> Self {
        let version = config.version.to_string();
        Self {
            config,
            status: RwLock::new(Status {
                version,
                level: StatusLevel::Available,
            }),
            api,
        }
    }

    /// Ensures that the user is an admin, otherwise returns an error (`403`).
    pub fn ensure_admin(&self, user: &User) -> Result<(), Error> {
        if user.subscription.get_features(&self.config).admin {
            Ok(())
        } else {
            Err(ErrorForbidden(anyhow!("Forbidden")))
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        api::Api,
        network::{Network, TokioDnsResolver},
        security::{create_webauthn, StoredCredentials},
        server::AppState,
        templates::create_templates,
        tests::{
            mock_config, mock_db, mock_network, mock_search_index, mock_user, MockUserBuilder,
        },
        users::{SubscriptionTier, UserSubscription},
    };
    use insta::assert_debug_snapshot;
    use lettre::{AsyncSmtpTransport, Tokio1Executor};
    use std::sync::Arc;
    use time::OffsetDateTime;

    pub async fn mock_app_state() -> anyhow::Result<AppState> {
        let config = mock_config()?;
        let webauthn = create_webauthn(&config)?;
        let api = Arc::new(Api::new(
            config,
            mock_db().await?,
            mock_search_index()?,
            // We should use a real network implementation in tests that rely on `AppState` being
            // extracted from `HttpRequest`, as types should match for the extraction to work.
            Network::new(
                TokioDnsResolver::create(),
                AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost(),
            ),
            webauthn,
            create_templates()?,
        ));

        Ok(AppState::new(api.config.clone(), api))
    }

    #[tokio::test]
    async fn can_detect_admin() -> anyhow::Result<()> {
        let config = mock_config()?;
        let webauthn = create_webauthn(&config)?;
        let api = Arc::new(Api::new(
            config,
            mock_db().await?,
            mock_search_index()?,
            mock_network(),
            webauthn,
            create_templates()?,
        ));

        let state = AppState::new(api.config.clone(), api);

        let user = mock_user()?;
        assert!(state.ensure_admin(&user).is_ok());

        let user = MockUserBuilder::new(
            user.id,
            &format!("dev-{}@secutils.dev", *user.id),
            &format!("dev-handle-{}", *user.id),
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Professional,
            started_at: OffsetDateTime::now_utc(),
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc()),
            trial_ends_at: None,
        })
        .build();

        assert_debug_snapshot!(
            state.ensure_admin(&user).unwrap_err(),
            @"Forbidden"
        );

        Ok(())
    }
}
