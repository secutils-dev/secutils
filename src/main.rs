#![deny(warnings)]

mod api;
mod config;
mod database;
mod directories;
mod error;
mod js_runtime;
mod logging;
mod network;
mod notifications;
mod scheduler;
mod search;
mod security;
mod server;
mod templates;
mod users;
mod utils;

use crate::config::{Config, RawConfig};
use anyhow::anyhow;
use clap::{crate_authors, crate_description, crate_version, value_parser, Arg, Command};

fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();
    structured_logger::Builder::new().init();

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

    let mut raw_config = RawConfig::read_from_file(
        matches
            .get_one::<String>("CONFIG")
            .ok_or_else(|| anyhow!("<CONFIG> argument is not provided."))?,
    )?;

    // CLI argument takes precedence.
    if let Some(port) = matches.get_one::<u16>("PORT") {
        raw_config.port = *port;
    }

    log::info!("Secutils.dev raw configuration: {raw_config:?}.");

    server::run(raw_config)
}

#[cfg(test)]
mod tests {
    use crate::{
        api::Api,
        config::{ComponentsConfig, Config, SchedulerJobsConfig, SmtpConfig, SubscriptionsConfig},
        database::Database,
        network::{DnsResolver, Network},
        search::SearchItem,
        security::StoredCredentials,
        users::{User, UserId},
        utils::web_scraping::{
            WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        },
    };
    use anyhow::anyhow;
    use cron::Schedule;
    use lettre::transport::stub::AsyncStubTransport;
    use std::{collections::HashMap, ops::Add, time::Duration};
    use tantivy::Index;
    use time::OffsetDateTime;
    use trust_dns_resolver::proto::rr::Record;
    use url::Url;

    use crate::{
        config::{JsRuntimeConfig, UtilsConfig},
        search::SearchIndex,
        security::create_webauthn,
        templates::create_templates,
        users::{SubscriptionTier, UserSubscription},
    };
    pub use crate::{logging::tests::*, network::tests::*, server::tests::*, utils::tests::*};
    use ctor::ctor;

    pub struct MockUserBuilder {
        user: User,
    }

    impl MockUserBuilder {
        pub fn new<I: Into<String>>(
            id: UserId,
            email: I,
            handle: I,
            credentials: StoredCredentials,
            created: OffsetDateTime,
        ) -> Self {
            let email = email.into();
            Self {
                user: User {
                    id,
                    email,
                    handle: handle.into(),
                    credentials,
                    created,
                    subscription: UserSubscription {
                        tier: SubscriptionTier::Ultimate,
                        started_at: created.add(Duration::from_secs(1)),
                        ends_at: None,
                        trial_started_at: None,
                        trial_ends_at: None,
                    },
                    activated: false,
                },
            }
        }

        pub fn set_activated(mut self) -> Self {
            self.user.activated = true;

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

    pub struct MockWebPageResourceBuilder {
        resource: WebPageResource,
    }

    impl MockWebPageResourceBuilder {
        pub fn with_content(data: WebPageResourceContentData, size: usize) -> Self {
            Self {
                resource: WebPageResource {
                    content: Some(WebPageResourceContent { data, size }),
                    url: None,
                    diff_status: None,
                },
            }
        }

        pub fn with_url(url: Url) -> Self {
            Self {
                resource: WebPageResource {
                    content: None,
                    url: Some(url),
                    diff_status: None,
                },
            }
        }

        pub fn set_content(mut self, data: WebPageResourceContentData, size: usize) -> Self {
            self.resource.content = Some(WebPageResourceContent { data, size });
            self
        }

        pub fn build(self) -> WebPageResource {
            self.resource
        }
    }

    pub async fn mock_db() -> anyhow::Result<Database> {
        Database::open(|| Ok("sqlite::memory:".to_string())).await
    }

    pub fn mock_search_index() -> anyhow::Result<SearchIndex> {
        SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))
    }

    pub fn mock_user() -> anyhow::Result<User> {
        mock_user_with_id(1)
    }

    pub fn mock_user_with_id<I: TryInto<UserId>>(id: I) -> anyhow::Result<User> {
        let id = id.try_into().map_err(|_| anyhow!("err"))?;
        Ok(MockUserBuilder::new(
            id,
            &format!("dev-{}@secutils.dev", *id),
            &format!("dev-handle-{}", *id),
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build())
    }

    pub fn mock_config() -> anyhow::Result<Config> {
        Ok(Config {
            public_url: Url::parse("http://localhost:1234")?,
            utils: UtilsConfig::default(),
            smtp: Some(SmtpConfig {
                username: "dev@secutils.dev".to_string(),
                password: "password".to_string(),
                address: "localhost".to_string(),
                catch_all: None,
            }),
            components: ComponentsConfig::default(),
            scheduler: SchedulerJobsConfig {
                web_page_trackers_schedule: Schedule::try_from("0 * 0 * * * *")?,
                web_page_trackers_fetch: Schedule::try_from("0 * 1 * * * *")?,
                notifications_send: Schedule::try_from("0 * 2 * * * *")?,
            },
            js_runtime: JsRuntimeConfig::default(),
            subscriptions: SubscriptionsConfig {
                manage_url: Some(Url::parse("http://localhost:1234/subscription")?),
                feature_overview_url: Some(Url::parse("http://localhost:1234/features")?),
            },
        })
    }

    pub fn mock_network() -> Network<MockResolver, AsyncStubTransport> {
        Network::new(MockResolver::new(), AsyncStubTransport::new_ok())
    }

    pub fn mock_network_with_records<const N: usize>(
        records: Vec<Record>,
    ) -> Network<MockResolver<N>, AsyncStubTransport> {
        Network::new(
            MockResolver::new_with_records::<N>(records),
            AsyncStubTransport::new_ok(),
        )
    }

    pub async fn mock_api() -> anyhow::Result<Api<MockResolver, AsyncStubTransport>> {
        mock_api_with_config(mock_config()?).await
    }

    pub async fn mock_api_with_config(
        config: Config,
    ) -> anyhow::Result<Api<MockResolver, AsyncStubTransport>> {
        let webauthn = create_webauthn(&config)?;
        Ok(Api::new(
            config,
            mock_db().await?,
            mock_search_index()?,
            mock_network(),
            webauthn,
            create_templates()?,
        ))
    }

    pub async fn mock_api_with_network<DR: DnsResolver>(
        network: Network<DR, AsyncStubTransport>,
    ) -> anyhow::Result<Api<DR, AsyncStubTransport>> {
        let config = mock_config()?;
        let webauthn = create_webauthn(&config)?;
        Ok(Api::new(
            config,
            mock_db().await?,
            mock_search_index()?,
            network,
            webauthn,
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

    pub fn mock_schedule_in_secs(secs: &[u64]) -> String {
        format!(
            "{} * * * * *",
            secs.iter()
                .map(|secs| {
                    OffsetDateTime::now_utc()
                        .add(Duration::from_secs(*secs))
                        .second()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    pub mod webauthn {
        pub const SERIALIZED_PASSKEY: &str = r#"{
          "cred": {
            "cred_id": "CVRiuJoJxH66qt-UWSnODqcnrVB4k_PFFHexRPqCroDAnaxn6_1Q01Y8VpYn8A2LcnpUeb6TBpTQaWUc4d1Mfg",
            "cred": {
              "type_": "ES256",
              "key": {
                "EC_EC2": {
                  "curve": "SECP256R1",
                  "x": "oRqUciz1zfd4bwCn-UaQ-KyfVDRfQHO5QIZl7PTPLDk",
                  "y": "5-fVS4_f1-EpqxAxVdhKJcXBxv1UcGpM0QB-XIR5gV4"
                }
              }
            },
            "counter": 0,
            "transports": null,
            "user_verified": false,
            "backup_eligible": false,
            "backup_state": false,
            "registration_policy": "preferred",
            "extensions": {
              "cred_protect": "NotRequested",
              "hmac_create_secret": "NotRequested",
              "appid": "NotRequested",
              "cred_props": "Ignored"
            },
            "attestation": {
              "data": "None",
              "metadata": "None"
            },
            "attestation_format": "None"
          }
       }"#;

        pub const SERIALIZED_REGISTRATION_STATE: &str = r#"{
            "rs": {
              "policy": "preferred",
              "exclude_credentials": [],
              "challenge": "36Fa2w5Kuv80nTzSHEvbA5rVE2Qm_x0ojjcPLYeB9RI",
              "credential_algorithms": [
                "ES256",
                "RS256"
              ],
              "require_resident_key": false,
              "authenticator_attachment": null,
              "extensions": {
                "uvm": true,
                "credProps": true
              },
              "experimental_allow_passkeys": true
            }
       }"#;

        pub const SERIALIZED_AUTHENTICATION_STATE: &str = r#"{
            "ast": {
              "credentials": [
                {
                  "cred_id": "fa900N3aOTRX0GThkakdjmLHsJdRTfvNMMxOgQ-hTy6W8-71w5zolJMDPq57ioYn1OM3fki5diO09kYyIBzQOg",
                  "cred": {
                    "type_": "ES256",
                    "key": {
                      "EC_EC2": {
                        "curve": "SECP256R1",
                        "x": "eLI4z21j77kGFYpblzcf5eapu3Wfk-H2eCbOX07EqEw",
                        "y": "AKYln3mCuIXuz6IsPT6pSU3qeAQkfDEOd5tVr0h--70"
                      }
                    }
                  },
                  "counter": 0,
                  "transports": null,
                  "user_verified": false,
                  "backup_eligible": false,
                  "backup_state": false,
                  "registration_policy": "preferred",
                  "extensions": {
                    "cred_protect": "NotRequested",
                    "hmac_create_secret": "NotRequested",
                    "appid": "NotRequested",
                    "cred_props": "Ignored"
                  },
                  "attestation": {
                    "data": "None",
                    "metadata": "None"
                  },
                  "attestation_format": "None"
                }
              ],
              "policy": "preferred",
              "challenge": "I2B0dgzCcgwkTyuUwA4yByFw5bBAl02axcEEoQNuSVM",
              "appid": null,
              "allow_backup_eligible_upgrade": true
            }
       }"#;
    }

    #[ctor]
    fn init_deno_runtime() {
        // Make sure deno runtime is initialized in the main thread before other tests.
        deno_core::JsRuntime::init_platform(None);
    }
}
