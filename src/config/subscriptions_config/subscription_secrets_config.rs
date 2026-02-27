use serde_derive::{Deserialize, Serialize};

/// Configuration for user secrets limits per subscription tier.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionSecretsConfig {
    /// Maximum number of secrets a user can create.
    pub max_secrets: usize,
}

impl Default for SubscriptionSecretsConfig {
    fn default() -> Self {
        Self { max_secrets: 100 }
    }
}
