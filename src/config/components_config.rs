use url::Url;

/// Configuration for the main Secutils.dev components.
#[derive(Clone, Debug)]
pub struct ComponentsConfig {
    /// The URL to access the Web Scrapper component.
    pub web_scrapper_url: Url,
    /// The current version of the search index component (typically incremented with Tantivy
    /// upgrades when there are breaking changes in the data or schema format).
    pub search_index_version: u16,
}
