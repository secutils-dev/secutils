#![deny(warnings)]

mod api;
mod config;
mod database;
mod directories;
mod error;
mod network;
mod notifications;
mod scheduler;
mod search;
mod security;
mod server;
mod templates;
mod users;
mod utils;

use crate::{
    config::{ComponentsConfig, Config, SchedulerJobsConfig, SmtpCatchAllConfig, SmtpConfig},
    server::WebhookUrlType,
};
use anyhow::{anyhow, Context};
use bytes::Buf;
use clap::{value_parser, Arg, ArgMatches, Command};
use cron::Schedule;
use lettre::message::Mailbox;
use std::str::FromStr;
use url::Url;

fn process_command(version: &str, matches: ArgMatches) -> Result<(), anyhow::Error> {
    let smtp_catch_all_config = match (
        matches.get_one::<String>("SMTP_CATCH_ALL_RECIPIENT"),
        matches.get_one::<String>("SMTP_CATCH_ALL_TEXT_MATCHER"),
    ) {
        (Some(recipient), Some(text_matcher)) => {
            let text_matcher = regex::Regex::new(text_matcher.as_str())
                .with_context(|| "Cannot parse SMTP catch-all text matcher.")?;
            Mailbox::from_str(recipient.as_str())
                .with_context(|| "Cannot parse SMTP catch-all recipient.")?;
            Some(SmtpCatchAllConfig {
                recipient: recipient.to_string(),
                text_matcher,
            })
        }
        (None, None) => None,
        (recipient, text_matcher) => {
            log::warn!(
                "SMTP catch-all config is not invalid: recipient ({:?}) and text_matcher ({:?}).",
                recipient,
                text_matcher
            );
            None
        }
    };
    let smtp_config = match (
        matches.get_one::<String>("SMTP_USERNAME"),
        matches.get_one::<String>("SMTP_PASSWORD"),
        matches.get_one::<String>("SMTP_ADDRESS"),
    ) {
        (Some(username), Some(password), Some(address)) => Some(SmtpConfig {
            username: username.to_string(),
            password: password.to_string(),
            address: address.to_string(),
            catch_all: smtp_catch_all_config,
        }),
        (username, password, address) => {
            log::warn!("SMTP config is not provided or invalid: username ({:?}), password ({:?}), address ({:?}).", username, password, address);
            None
        }
    };

    let config = Config {
        version: version.to_owned(),
        smtp: smtp_config,
        http_port: *matches
            .get_one("HTTP_PORT")
            .ok_or_else(|| anyhow!("<HTTP_PORT> argument is not provided."))?,
        public_url: matches
            .get_one::<String>("PUBLIC_URL")
            .ok_or_else(|| anyhow!("<PUBLIC_URL> argument is not provided."))
            .and_then(|public_url| {
                Url::parse(public_url)
                    .with_context(|| "Cannot parse public URL parameter.".to_string())
            })?,
        webhook_url_type: matches
            .get_one::<String>("WEBHOOK_URL_TYPE")
            .ok_or_else(|| anyhow!("<WEBHOOK_URL_TYPE> argument is not provided."))
            .and_then(|webhook_url_type| {
                WebhookUrlType::from_str(webhook_url_type)
                    .with_context(|| "Cannot parse webhook URL type parameter.".to_string())
            })?,
        components: ComponentsConfig {
            web_scraper_url: matches
                .get_one::<String>("COMPONENT_WEB_SCRAPER_URL")
                .ok_or_else(|| anyhow!("<COMPONENT_WEB_SCRAPER_URL> argument is not provided."))
                .and_then(|url| {
                    Url::parse(url)
                        .with_context(|| "Cannot parse Web Scraper URL parameter.".to_string())
                })?,
            search_index_version: 3,
        },
        jobs: SchedulerJobsConfig {
            web_page_trackers_schedule: matches
                .get_one::<String>("JOBS_WEB_PAGE_TRACKERS_SCHEDULE")
                .ok_or_else(|| {
                    anyhow!("<JOBS_WEB_PAGE_TRACKERS_SCHEDULE> argument is not provided.")
                })
                .and_then(|schedule| {
                    Schedule::try_from(schedule.as_str())
                        .with_context(|| "Cannot parse web page trackers schedule job schedule.")
                })?,
            web_page_trackers_fetch: matches
                .get_one::<String>("JOBS_WEB_PAGE_TRACKERS_FETCH")
                .ok_or_else(|| anyhow!("<JOBS_WEB_PAGE_TRACKERS_FETCH> argument is not provided."))
                .and_then(|schedule| {
                    Schedule::try_from(schedule.as_str())
                        .with_context(|| "Cannot parse web page trackers fetch job schedule.")
                })?,
            notifications_send: matches
                .get_one::<String>("JOBS_NOTIFICATIONS_SEND")
                .ok_or_else(|| anyhow!("<JOBS_NOTIFICATIONS_SEND> argument is not provided."))
                .and_then(|schedule| {
                    Schedule::try_from(schedule.as_str())
                        .with_context(|| "Cannot parse notifications send job schedule.")
                })?,
        },
    };

    let session_key = matches
        .get_one::<String>("SESSION_KEY")
        .ok_or_else(|| anyhow!("<SESSION_KEY> argument is not provided."))
        .and_then(|value| {
            let mut session_key = [0; 64];
            if value.as_bytes().len() != session_key.len() {
                Err(anyhow!(format!(
                    "<SESSION_KEY> argument should be {} bytes long.",
                    session_key.len()
                )))
            } else {
                value.as_bytes().copy_to_slice(&mut session_key);
                Ok(session_key)
            }
        })?;

    let secure_cookies = !matches.get_flag("SESSION_USE_INSECURE_COOKIES");

    let builtin_users = matches
        .get_one::<String>("BUILTIN_USERS")
        .map(|value| value.to_string());

    server::run(config, session_key, secure_cookies, builtin_users)
}

fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();
    env_logger::init();

    let version = env!("CARGO_PKG_VERSION");

    let matches = Command::new("Secutils.dev API server")
        .version(version)
        .author("Secutils <dev@secutils.dev")
        .about("Secutils.dev API server")
        .arg(
            Arg::new("SESSION_KEY")
                .long("session-key")
                .global(true)
                .env("SECUTILS_SESSION_KEY")
                .help("Session encryption key."),
        )
        .arg(
            Arg::new("SESSION_USE_INSECURE_COOKIES")
                .long("use-insecure-cookies")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .env("SECUTILS_SESSION_USE_INSECURE_COOKIES")
                .help("Indicates that server shouldn't set `Secure` flag on the session cookie (do not use in production)."),
        )
        .arg(
            Arg::new("SMTP_USERNAME")
                .long("smtp-username")
                .global(true)
                .env("SECUTILS_SMTP_USERNAME")
                .help("Username to use to authenticate to the SMTP server."),
        )
        .arg(
            Arg::new("SMTP_PASSWORD")
                .long("smtp-password")
                .global(true)
                .env("SECUTILS_SMTP_PASSWORD")
                .help("Password to use to authenticate to the SMTP server."),
        )
        .arg(
            Arg::new("SMTP_ADDRESS")
                .long("smtp-address")
                .global(true)
                .env("SECUTILS_SMTP_ADDRESS")
                .help("Address of the SMTP server."),
        )
        .arg(
            Arg::new("SMTP_CATCH_ALL_RECIPIENT")
                .long("smtp-catch-all-recipient")
                .global(true)
                .env("SECUTILS_SMTP_CATCH_ALL_RECIPIENT")
                .requires("SMTP_CATCH_ALL_TEXT_MATCHER")
                .help("Address of the catch-all email recipient (used for troubleshooting only)."),
        )
        .arg(
            Arg::new("SMTP_CATCH_ALL_TEXT_MATCHER")
                .long("smtp-catch-all-text-matcher")
                .global(true)
                .env("SECUTILS_SMTP_CATCH_ALL_TEXT_MATCHER")
                .requires("SMTP_CATCH_ALL_RECIPIENT")
                .help("Email text should match specified regular expression to be sent to catch-all recipient (used for troubleshooting only)."),
        )
        .arg(
            Arg::new("BUILTIN_USERS")
                .long("builtin-users")
                .global(true)
                .env("SECUTILS_BUILTIN_USERS")
                .help("List of the builtin users in a single string format."),
        )
        .arg(
            Arg::new("HTTP_PORT")
                .value_parser(value_parser!(u16))
                .short('p')
                .long("http-port")
                .default_value("7070")
                .help("Defines a TCP port to listen on."),
        )
        .arg(
            Arg::new("PUBLIC_URL")
                .long("public-url")
                .global(true)
                .env("SECUTILS_PUBLIC_URL")
                .default_value("http://localhost:7070")
                .help("External/public URL through which service is being accessed."),
        )
        .arg(
            Arg::new("WEBHOOK_URL_TYPE")
                .long("webhook-url-type")
                .global(true)
                .env("SECUTILS_WEBHOOK_URL_TYPE")
                .default_value("path")
                .value_names(["path", "subdomain"])
                .help("Describes how Secutils.dev WebUI should construct webhook URLs. The server supports all types of URL simultaneously."),
        )
        .arg(
            Arg::new("COMPONENT_WEB_SCRAPER_URL")
                .long("component-web-scraper-url")
                .global(true)
                .env("SECUTILS_COMPONENT_WEB_SCRAPER_URL")
                .default_value("http://localhost:7272")
                .help("The URL to access the Web Scraper component."),
        )
        .arg(
            Arg::new("JOBS_WEB_PAGE_TRACKERS_SCHEDULE")
                .long("jobs-web-page-trackers-schedule")
                .global(true)
                .env("SECUTILS_JOBS_WEB_PAGE_TRACKERS_SCHEDULE")
                .default_value("0 * * * * * *")
                .help("The cron schedule to use for the web page trackers schedule job."),
        )
        .arg(
            Arg::new("JOBS_WEB_PAGE_TRACKERS_FETCH")
                .long("jobs-web-page-trackers-fetch")
                .global(true)
                .env("SECUTILS_JOBS_WEB_PAGE_TRACKERS_FETCH")
                .default_value("0 * * * * * *")
                .help("The cron schedule to use for the web page trackers fetch job."),
        ).arg(
        Arg::new("JOBS_NOTIFICATIONS_SEND")
            .long("jobs-notifications-send")
            .global(true)
            .env("SECUTILS_JOBS_NOTIFICATIONS_SEND")
            .default_value("0/30 * * * * * *")
            .help("The cron schedule to use for the notifications send job."),
        )
        .get_matches();

    process_command(version, matches)
}

#[cfg(test)]
mod tests {
    use crate::{
        api::Api,
        config::{ComponentsConfig, Config, SchedulerJobsConfig, SmtpConfig},
        database::Database,
        network::{DnsResolver, Network},
        search::SearchItem,
        security::StoredCredentials,
        users::{User, UserId},
        utils::{WebPageResource, WebPageResourceContent, WebPageResourceContentData},
    };
    use anyhow::anyhow;
    use cron::Schedule;
    use lettre::transport::stub::AsyncStubTransport;
    use std::{
        collections::{HashMap, HashSet},
        ops::Add,
        time::Duration,
    };
    use tantivy::Index;
    use time::OffsetDateTime;
    use trust_dns_resolver::proto::rr::Record;
    use url::Url;

    pub use crate::{network::tests::*, server::tests::*, utils::tests::*};
    use crate::{search::SearchIndex, server::WebhookUrlType, templates::create_templates};

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
                    roles: HashSet::new(),
                    activated: false,
                },
            }
        }

        pub fn set_activated(mut self) -> Self {
            self.user.activated = true;

            self
        }

        pub fn add_role<R: AsRef<str>>(mut self, role: R) -> Self {
            self.user.roles.insert(role.as_ref().to_lowercase());
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
            OffsetDateTime::now_utc(),
        )
        .build())
    }

    pub fn mock_config() -> anyhow::Result<Config> {
        Ok(Config {
            version: "1.0.0".to_string(),
            http_port: 1234,
            public_url: Url::parse("http://localhost:1234")?,
            webhook_url_type: WebhookUrlType::Subdomain,
            smtp: Some(SmtpConfig {
                username: "dev@secutils.dev".to_string(),
                password: "password".to_string(),
                address: "localhost".to_string(),
                catch_all: None,
            }),
            components: ComponentsConfig {
                web_scraper_url: Url::parse("http://localhost:7272")?,
                search_index_version: 3,
            },
            jobs: SchedulerJobsConfig {
                web_page_trackers_schedule: Schedule::try_from("0 * 0 * * * *")?,
                web_page_trackers_fetch: Schedule::try_from("0 * 1 * * * *")?,
                notifications_send: Schedule::try_from("0 * 2 * * * *")?,
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
        Ok(Api::new(
            config,
            mock_db().await?,
            mock_search_index()?,
            mock_network(),
            create_templates()?,
        ))
    }

    pub async fn mock_api_with_network<DR: DnsResolver>(
        network: Network<DR, AsyncStubTransport>,
    ) -> anyhow::Result<Api<DR, AsyncStubTransport>> {
        Ok(Api::new(
            mock_config()?,
            mock_db().await?,
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
}
