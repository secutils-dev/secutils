use serde_derive::{Deserialize, Serialize};

/// Configuration for user scripts limits per subscription tier.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionScriptsConfig {
    /// Maximum number of scripts a user can create.
    pub max_scripts: usize,
}

impl Default for SubscriptionScriptsConfig {
    fn default() -> Self {
        Self { max_scripts: 100 }
    }
}
