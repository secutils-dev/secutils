use crate::server::WebhookUrlType;
use serde_derive::{Deserialize, Serialize};

/// Configuration for the JS runtime (Deno).
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct UtilsConfig {
    /// Describes the preferred way to construct webhook URLs.
    pub webhook_url_type: WebhookUrlType,
}

impl Default for UtilsConfig {
    fn default() -> Self {
        Self {
            webhook_url_type: WebhookUrlType::Subdomain,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{config::UtilsConfig, server::WebhookUrlType};
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(UtilsConfig::default(), @"webhook-url-type = 'subdomain'");
    }

    #[test]
    fn deserialization() {
        let config: UtilsConfig = toml::from_str(r#"webhook-url-type = 'path'"#).unwrap();
        assert_eq!(
            config,
            UtilsConfig {
                webhook_url_type: WebhookUrlType::Path
            }
        );
    }
}
