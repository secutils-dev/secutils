use url::Url;

/// Configuration related to the Secutils.dev subscriptions.
#[derive(Clone, Debug)]
pub struct SubscriptionsConfig {
    /// The URL to access the subscription management page.
    pub manage_url: Option<Url>,
    /// The URL to access the feature overview page.
    pub feature_overview_url: Option<Url>,
}
