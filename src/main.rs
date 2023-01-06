#![deny(warnings)]

mod api;
mod config;
mod datastore;
mod error;
mod file_cache;
mod search;
mod server;
mod users;
mod utils;

use crate::config::{Config, SmtpConfig};
use anyhow::anyhow;
use bytes::Buf;
use clap::{value_parser, Arg, ArgMatches, Command};

fn process_command(matches: ArgMatches) -> Result<(), anyhow::Error> {
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
            log::error!("SMTP config is not provided or invalid: username ({:?}), password ({:?}), address ({:?}).", username, password, address);
            None
        }
    };

    let config = Config {
        smtp: smtp_config,
        http_port: *matches
            .get_one("HTTP_PORT")
            .ok_or_else(|| anyhow!("<HTTP_PORT> argument is not provided."))?,
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
    dotenv::dotenv().ok();
    env_logger::init();

    let matches = Command::new("Secutils.dev API server")
        .version("0.1.0")
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
        .get_matches();

    process_command(matches)
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::initialize_index,
        search::SearchItem,
        users::{User, UserProfile},
    };
    use std::collections::HashSet;
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
            email: I,
            handle: I,
            password_hash: I,
            created: OffsetDateTime,
        ) -> Self {
            let email = email.into();
            Self {
                user: User {
                    email,
                    handle: handle.into(),
                    password_hash: password_hash.into(),
                    created,
                    roles: HashSet::new(),
                    profile: None,
                    activation_code: None,
                },
            }
        }

        pub fn set_activation_code<I: Into<String>>(mut self, activation_code: I) -> Self {
            self.user.activation_code = Some(activation_code.into());

            self
        }

        pub fn set_profile(mut self, profile: UserProfile) -> Self {
            self.user.profile = Some(profile);
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
        pub fn new<I: Into<String>>(id: I, content: I, timestamp: OffsetDateTime) -> Self {
            Self {
                item: SearchItem {
                    id: id.into(),
                    content: content.into(),
                    user_handle: None,
                    timestamp,
                },
            }
        }

        pub fn set_user_handle<I: Into<String>>(mut self, user_handle: I) -> Self {
            self.item.user_handle = Some(user_handle.into());

            self
        }

        pub fn build(self) -> SearchItem {
            self.item
        }
    }
}
