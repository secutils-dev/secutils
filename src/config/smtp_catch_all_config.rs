use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

/// Configuration for the SMTP catch-all functionality.
#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SmtpCatchAllConfig {
    /// Address of the catch-all email recipient.
    pub recipient: String,
    /// Email will be sent to the catch-all recipient instead of original one only if the email text
    /// matches regular expression specified in `text_matcher`.
    #[serde_as(as = "DisplayFromStr")]
    pub text_matcher: Regex,
}

#[cfg(test)]
mod tests {
    use crate::config::SmtpCatchAllConfig;
    use insta::{assert_debug_snapshot, assert_toml_snapshot};
    use regex::Regex;

    #[test]
    fn serialization() {
        assert_toml_snapshot!(SmtpCatchAllConfig {
            recipient: "test@secutils.dev".to_string(),
            text_matcher: Regex::new(r"test").unwrap(),
        }, @r###"
        recipient = 'test@secutils.dev'
        text_matcher = 'test'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SmtpCatchAllConfig = toml::from_str(
            r#"
        recipient = 'test@secutils.dev'
        text_matcher = 'test'
    "#,
        )
        .unwrap();
        assert_debug_snapshot!(config, @r###"
        SmtpCatchAllConfig {
            recipient: "test@secutils.dev",
            text_matcher: Regex(
                "test",
            ),
        }
        "###);
    }
}
