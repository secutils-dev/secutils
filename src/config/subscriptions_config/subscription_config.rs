use crate::config::{
    SubscriptionCertificatesConfig, SubscriptionWebScrapingConfig, SubscriptionWebSecurityConfig,
    SubscriptionWebhooksConfig,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SubscriptionConfig {
    /// The config managing the webhooks utilities for a particular subscription.
    pub webhooks: SubscriptionWebhooksConfig,
    /// The config managing the web scraping utilities for a particular subscription.
    pub web_scraping: SubscriptionWebScrapingConfig,
    /// The config managing the certificates utilities for a particular subscription.
    pub certificates: SubscriptionCertificatesConfig,
    /// The config managing the web security utilities for a particular subscription.
    pub web_security: SubscriptionWebSecurityConfig,
}
