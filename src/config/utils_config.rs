use crate::server::WebhookUrlType;
use serde_derive::{Deserialize, Serialize};

/// Configuration for the utility functions.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct UtilsConfig {
    /// Describes the preferred way to construct webhook URLs.
    pub webhook_url_type: WebhookUrlType,
    /// Number of unchanged context lines surrounding each change hunk in unified diff output.
    pub diff_context_radius: usize,
}

impl Default for UtilsConfig {
    fn default() -> Self {
        Self {
            webhook_url_type: WebhookUrlType::Subdomain,
            diff_context_radius: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{config::UtilsConfig, server::WebhookUrlType};
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(UtilsConfig::default(), @r###"
        webhook_url_type = 'subdomain'
        diff_context_radius = 3
        "###);
    }

    #[test]
    fn deserialization() {
        let config: UtilsConfig = toml::from_str(
            r#"webhook_url_type = 'path'
diff_context_radius = 5"#,
        )
        .unwrap();
        assert_eq!(
            config,
            UtilsConfig {
                webhook_url_type: WebhookUrlType::Path,
                diff_context_radius: 5,
            }
        );
    }
}
