use serde_derive::{Deserialize, Serialize};
use url::Url;

/// Configuration for the main Secutils.dev components.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ComponentsConfig {
    /// The URL to access the Kratos component.
    pub kratos_url: Url,
    /// The URL to access the Kratos component (admin).
    pub kratos_admin_url: Url,
    /// The URL to access the Web Scraper component.
    pub web_scraper_url: Url,
    /// The current version of the search index component (typically incremented with Tantivy
    /// upgrades when there are breaking changes in the data or schema format).
    pub search_index_version: u16,
}

impl Default for ComponentsConfig {
    fn default() -> Self {
        Self {
            kratos_url: Url::parse("http://localhost:4433")
                .expect("Cannot parse Kratos URL parameter."),
            kratos_admin_url: Url::parse("http://localhost:4434")
                .expect("Cannot parse Kratos Admin URL parameter."),
            web_scraper_url: Url::parse("http://localhost:7272")
                .expect("Cannot parse Web Scraper URL parameter."),
            search_index_version: 4,
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
        kratos_url = 'http://localhost:4433/'
        kratos_admin_url = 'http://localhost:4434/'
        web_scraper_url = 'http://localhost:7272/'
        search_index_version = 4
        "###);
    }

    #[test]
    fn deserialization() {
        let config: ComponentsConfig = toml::from_str(
            r#"
        kratos_url = 'http://localhost:4433/'
        kratos_admin_url = 'http://localhost:4434/'
        web_scraper_url = 'http://localhost:7272/'
        search_index_version = 4
    "#,
        )
        .unwrap();
        assert_eq!(config, ComponentsConfig::default());
    }
}
