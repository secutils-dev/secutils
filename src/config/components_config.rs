use url::Url;

/// Configuration for the components that are deployed separately.
#[derive(Clone, Debug)]
pub struct ComponentsConfig {
    /// The URL to access the Web Scrapper component.
    pub web_scrapper_url: Url,
}
