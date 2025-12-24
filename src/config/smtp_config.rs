use crate::config::SmtpCatchAllConfig;
use serde_derive::{Deserialize, Serialize};

/// Configuration for the SMTP functionality.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SmtpConfig {
    /// Username to use to authenticate to the SMTP server.
    pub username: String,
    /// Password to use to authenticate to the SMTP server.
    pub password: String,
    /// Address of the SMTP server.
    pub address: String,
    /// Optional configuration for catch-all email recipient (used for troubleshooting only).
    pub catch_all: Option<SmtpCatchAllConfig>,
}

#[cfg(test)]
mod tests {
    use crate::config::{SmtpCatchAllConfig, SmtpConfig};
    use insta::{assert_debug_snapshot, assert_toml_snapshot};
    use regex::Regex;

    #[test]
    fn serialization() {
        let config = SmtpConfig {
            username: "test@secutils.dev".to_string(),
            password: "password".to_string(),
            address: "smtp.secutils.dev".to_string(),
            catch_all: None,
        };
        assert_toml_snapshot!(config, @r###"
        username = 'test@secutils.dev'
        password = 'password'
        address = 'smtp.secutils.dev'
        "###);

        let config = SmtpConfig {
            username: "test@secutils.dev".to_string(),
            password: "password".to_string(),
            address: "smtp.secutils.dev".to_string(),
            catch_all: Some(SmtpCatchAllConfig {
                recipient: "test@secutils.dev".to_string(),
                text_matcher: Regex::new(r"test").unwrap(),
            }),
        };
        assert_toml_snapshot!(config, @r###"
        username = 'test@secutils.dev'
        password = 'password'
        address = 'smtp.secutils.dev'
        catch_all = { recipient = 'test@secutils.dev', text_matcher = 'test' }
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SmtpConfig = toml::from_str(
            r#"
        username = 'test@secutils.dev'
        password = 'password'
        address = 'smtp.secutils.dev'

        [catch_all]
        recipient = 'test@secutils.dev'
        text_matcher = 'test'
    "#,
        )
        .unwrap();
        assert_debug_snapshot!(config, @r###"
        SmtpConfig {
            username: "test@secutils.dev",
            password: "password",
            address: "smtp.secutils.dev",
            catch_all: Some(
                SmtpCatchAllConfig {
                    recipient: "test@secutils.dev",
                    text_matcher: Regex(
                        "test",
                    ),
                },
            ),
        }
        "###);
    }
}
