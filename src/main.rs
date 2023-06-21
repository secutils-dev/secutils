#![deny(warnings)]

mod api;
mod authentication;
mod config;
mod datastore;
mod directories;
mod error;
mod search;
mod server;
mod users;
mod utils;

use crate::config::{ComponentsConfig, Config, SmtpConfig};
use anyhow::{anyhow, Context};
use bytes::Buf;
use clap::{value_parser, Arg, ArgMatches, Command};
use url::Url;

fn process_command(version: &str, matches: ArgMatches) -> Result<(), anyhow::Error> {
    let smtp_config = match (
        matches.get_one::<String>("SMTP_USERNAME"),
        matches.get_one::<String>("SMTP_PASSWORD"),
        matches.get_one::<String>("SMTP_ADDRESS"),
        matches.get_one::<String>("SMTP_CATCH_ALL_RECIPIENT"),
    ) {
        (Some(username), Some(password), Some(address), recipient) => Some(SmtpConfig {
            username: username.to_string(),
            password: password.to_string(),
            address: address.to_string(),
            catch_all_recipient: recipient.map(|value| value.to_string()),
        }),
        (username, password, address, _) => {
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
        components: ComponentsConfig {
            web_scraper_url: matches
                .get_one::<String>("COMPONENT_WEB_SCRAPER_URL")
                .ok_or_else(|| anyhow!("<COMPONENT_WEB_SCRAPER_URL> argument is not provided."))
                .and_then(|url| {
                    Url::parse(url)
                        .with_context(|| "Cannot parse Web Scraper URL parameter.".to_string())
                })?,
            search_index_version: 1,
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
                .help("Address of the email recipient (used for debug only)."),
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
            Arg::new("COMPONENT_WEB_SCRAPER_URL")
                .long("component-web-scraper-url")
                .global(true)
                .env("SECUTILS_COMPONENT_WEB_SCRAPER_URL")
                .default_value("http://localhost:7272")
                .help("The URL to access the Web Scraper component."),
        )
        .get_matches();

    process_command(version, matches)
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::StoredCredentials,
        datastore::{initialize_index, PrimaryDb},
        search::SearchItem,
        users::{User, UserId},
    };
    use std::collections::{HashMap, HashSet};
    use tantivy::{schema::Schema, Index, IndexReader};
    use time::OffsetDateTime;

    pub fn open_index(schema: Schema) -> anyhow::Result<(Index, IndexReader)> {
        initialize_index(Index::create_in_ram(schema))
    }

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

    pub async fn mock_db() -> anyhow::Result<PrimaryDb> {
        PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await
    }

    pub fn mock_user() -> User {
        MockUserBuilder::new(
            UserId(1),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .build()
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
