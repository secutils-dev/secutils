#![deny(warnings)]

mod api;
mod config;
mod database;
mod directories;
mod error;
mod js_runtime;
mod network;
mod notifications;
mod retrack;
mod scheduler;
mod search;
mod security;
mod server;
mod templates;
mod users;
mod utils;

use crate::config::{Config, RawConfig};
use anyhow::anyhow;
use clap::{Arg, Command, crate_authors, crate_description, crate_version, value_parser};
use std::env;
use tracing::info;

fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();

    if env::var("RUST_LOG_FORMAT").is_ok_and(|format| format == "json") {
        tracing_subscriber::fmt().json().flatten_event(true).init();
    } else {
        tracing_subscriber::fmt::init();
    }

    let matches = Command::new("Secutils.dev API server")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::new("CONFIG")
                .env("SECUTILS_CONFIG")
                .short('c')
                .long("config")
                .default_value("secutils.toml")
                .help("Path to the application configuration file."),
        )
        .arg(
            Arg::new("PORT")
                .env("SECUTILS_PORT")
                .short('p')
                .long("port")
                .value_parser(value_parser!(u16))
                .help("Defines a TCP port to listen on."),
        )
        .get_matches();

    let raw_config = RawConfig::read_from_file(
        matches
            .get_one::<String>("CONFIG")
            .ok_or_else(|| anyhow!("<CONFIG> argument is not provided."))?,
    )?;

    info!("Secutils.dev raw configuration: {raw_config:?}.");

    // CLI argument takes precedence.
    let http_port = matches
        .get_one::<u16>("PORT")
        .copied()
        .unwrap_or(raw_config.port);
    server::run(Config::from(raw_config), http_port)
}

#[cfg(test)]
mod tests {
    use crate::{
        api::Api,
        config::{Config, SchedulerJobsConfig, SmtpConfig, SubscriptionsConfig},
        database::Database,
        network::{DnsResolver, Network},
        search::SearchItem,
        users::{User, UserId},
    };
    use lettre::transport::stub::AsyncStubTransport;
    use std::{collections::HashMap, ops::Add, time::Duration};
    use tantivy::Index;
    use time::OffsetDateTime;
    use trust_dns_resolver::proto::rr::Record;
    use url::Url;

    use crate::{
        config::SubscriptionConfig,
        search::SearchIndex,
        templates::create_templates,
        users::{SubscriptionTier, UserSubscription},
    };
    pub use crate::{network::tests::*, scheduler::tests::*, server::tests::*, utils::tests::*};
    use ctor::ctor;
    use reqwest::Client;
    use reqwest_middleware::ClientBuilder;
    use sqlx::{PgPool, postgres::PgDatabaseError};
    use uuid::uuid;

    pub struct MockUserBuilder {
        user: User,
    }

    impl MockUserBuilder {
        pub fn new<I: Into<String>>(
            id: UserId,
            email: I,
            handle: I,
            created_at: OffsetDateTime,
        ) -> Self {
            let email = email.into();
            Self {
                user: User {
                    id,
                    email,
                    handle: handle.into(),
                    created_at,
                    subscription: UserSubscription {
                        tier: SubscriptionTier::Ultimate,
                        started_at: created_at.add(Duration::from_secs(1)),
                        ends_at: None,
                        trial_started_at: None,
                        trial_ends_at: None,
                    },
                    is_activated: false,
                    is_operator: false,
                },
            }
        }

        pub fn set_is_activated(mut self) -> Self {
            self.user.is_activated = true;

            self
        }

        pub fn set_subscription(mut self, subscription: UserSubscription) -> Self {
            self.user.subscription = subscription;
            self
        }

        pub fn build(self) -> User {
            self.user
        }
    }

    pub struct MockSearchItemBuilder {
        item: SearchItem,
    }

    impl MockSearchItemBuilder {
        pub fn new<I: Into<String>>(
            id: u64,
            label: I,
            category: I,
            timestamp: OffsetDateTime,
        ) -> Self {
            Self {
                item: SearchItem {
                    id,
                    label: label.into(),
                    category: category.into(),
                    sub_category: None,
                    keywords: None,
                    user_id: None,
                    meta: None,
                    timestamp,
                },
            }
        }

        pub fn set_sub_category<I: Into<String>>(mut self, sub_category: I) -> Self {
            self.item.sub_category = Some(sub_category.into());
            self
        }

        pub fn set_keywords<I: Into<String>>(mut self, keywords: I) -> Self {
            self.item.keywords = Some(keywords.into());
            self
        }

        pub fn set_user_id(mut self, user_id: UserId) -> Self {
            self.item.user_id = Some(user_id);
            self
        }

        pub fn set_meta<I: Into<HashMap<String, String>>>(mut self, meta: I) -> Self {
            self.item.meta = Some(meta.into());
            self
        }

        pub fn build(self) -> SearchItem {
            self.item
        }
    }

    pub fn to_database_error(err: anyhow::Error) -> anyhow::Result<Box<PgDatabaseError>> {
        Ok(err
            .downcast::<sqlx::Error>()?
            .into_database_error()
            .unwrap()
            .downcast::<PgDatabaseError>())
    }

    pub fn mock_search_index() -> anyhow::Result<SearchIndex> {
        SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))
    }

    pub fn mock_user() -> anyhow::Result<User> {
        mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000001"))
    }

    pub fn mock_user_with_id<I: Into<UserId>>(id: I) -> anyhow::Result<User> {
        let id = id.into();
        Ok(MockUserBuilder::new(
            id,
            &format!("dev-{}@secutils.dev", *id),
            &format!("devhandle{}", *id.as_simple()),
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build())
    }

    pub fn mock_config() -> anyhow::Result<Config> {
        Ok(Config {
            public_url: Url::parse("https://secutils.dev")?,
            db: Default::default(),
            http: Default::default(),
            utils: Default::default(),
            smtp: Some(SmtpConfig {
                username: "dev@secutils.dev".to_string(),
                password: "password".to_string(),
                address: "localhost".to_string(),
                catch_all: None,
            }),
            components: Default::default(),
            scheduler: SchedulerJobsConfig {
                notifications_send: "0 * 2 * * *".to_string(),
            },
            security: Default::default(),
            subscriptions: SubscriptionsConfig {
                manage_url: Some(Url::parse("http://localhost:1234/subscription")?),
                feature_overview_url: Some(Url::parse("http://localhost:1234/features")?),
                basic: SubscriptionConfig::default(),
                standard: SubscriptionConfig::default(),
                professional: SubscriptionConfig::default(),
                ultimate: SubscriptionConfig::default(),
            },
            retrack: Default::default(),
        })
    }

    pub fn mock_network() -> Network<MockResolver, AsyncStubTransport> {
        Network::new(
            MockResolver::new(),
            AsyncStubTransport::new_ok(),
            ClientBuilder::new(Client::builder().build().unwrap()).build(),
        )
    }

    pub fn mock_network_with_records<const N: usize>(
        records: Vec<Record>,
    ) -> Network<MockResolver<N>, AsyncStubTransport> {
        Network::new(
            MockResolver::new_with_records::<N>(records),
            AsyncStubTransport::new_ok(),
            ClientBuilder::new(Client::builder().build().unwrap()).build(),
        )
    }

    pub async fn mock_api(pool: PgPool) -> anyhow::Result<Api<MockResolver, AsyncStubTransport>> {
        mock_api_with_config(pool, mock_config()?).await
    }

    pub async fn mock_api_with_config(
        pool: PgPool,
        config: Config,
    ) -> anyhow::Result<Api<MockResolver, AsyncStubTransport>> {
        Ok(Api::new(
            config,
            Database::create(pool).await?,
            mock_search_index()?,
            mock_network(),
            create_templates()?,
        ))
    }

    pub async fn mock_api_with_network<DR: DnsResolver>(
        pool: PgPool,
        network: Network<DR, AsyncStubTransport>,
    ) -> anyhow::Result<Api<DR, AsyncStubTransport>> {
        Ok(Api::new(
            mock_config()?,
            Database::create(pool).await?,
            mock_search_index()?,
            network,
            create_templates()?,
        ))
    }

    pub fn mock_schedule_in_sec(secs: u64) -> String {
        format!(
            "{} * * * * *",
            OffsetDateTime::now_utc()
                .add(Duration::from_secs(secs))
                .second()
        )
    }

    #[ctor]
    fn init_deno_runtime() {
        // Make sure deno runtime is initialized in the main thread before other tests.
        deno_core::JsRuntime::init_platform(None);
    }
}
