use serde_derive::{Deserialize, Serialize};
use url::Url;

/// Configuration related to the Secutils.dev subscriptions.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SubscriptionsConfig {
    /// The URL to access the subscription management page.
    pub manage_url: Option<Url>,
    /// The URL to access the feature overview page.
    pub feature_overview_url: Option<Url>,
}

#[cfg(test)]
mod tests {
    use crate::config::SubscriptionsConfig;
    use insta::assert_toml_snapshot;
    use url::Url;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SubscriptionsConfig::default(), @"");

        let config = SubscriptionsConfig {
            manage_url: Some(Url::parse("http://localhost:7272").unwrap()),
            feature_overview_url: Some(Url::parse("http://localhost:7272").unwrap()),
        };
        assert_toml_snapshot!(config, @r###"
        manage-url = 'http://localhost:7272/'
        feature-overview-url = 'http://localhost:7272/'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionsConfig = toml::from_str(
            r#"
        manage-url = 'http://localhost:7272/'
        feature-overview-url = 'http://localhost:7272/'
    "#,
        )
        .unwrap();
        assert_eq!(
            config,
            SubscriptionsConfig {
                manage_url: Some(Url::parse("http://localhost:7272").unwrap()),
                feature_overview_url: Some(Url::parse("http://localhost:7272").unwrap()),
            }
        );
    }
}
