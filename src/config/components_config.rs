use serde_derive::{Deserialize, Serialize};
use url::Url;

/// Configuration for the main Secutils.dev components.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ComponentsConfig {
    /// The URL to access the Web Scraper component.
    pub web_scraper_url: Url,
    /// The current version of the search index component (typically incremented with Tantivy
    /// upgrades when there are breaking changes in the data or schema format).
    pub search_index_version: u16,
}

impl Default for ComponentsConfig {
    fn default() -> Self {
        Self {
            web_scraper_url: Url::parse("http://localhost:7272")
                .expect("Cannot parse Web Scraper URL parameter."),
            search_index_version: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::ComponentsConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(ComponentsConfig::default(), @r###"
        web-scraper-url = 'http://localhost:7272/'
        search-index-version = 3
        "###);
    }

    #[test]
    fn deserialization() {
        let config: ComponentsConfig = toml::from_str(
            r#"
        web-scraper-url = 'http://localhost:7272/'
        search-index-version = 3
    "#,
        )
        .unwrap();
        assert_eq!(config, ComponentsConfig::default());
    }
}
