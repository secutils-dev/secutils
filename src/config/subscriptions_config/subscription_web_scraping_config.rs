use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::{collections::HashSet, time::Duration};

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionWebScrapingConfig {
    /// The number of trackers (content, resources etc.) available to a particular subscription.
    pub trackers: usize,
    /// The number of tracker revisions per tracker that retained for a particular subscription.
    pub tracker_revisions: usize,
    /// The list of allowed schedules for the trackers for a particular subscription.
    pub tracker_schedules: Option<HashSet<String>>,
    /// The minimum interval between two consequent scheduled tracker checks.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub min_schedule_interval: Duration,
}

impl Default for SubscriptionWebScrapingConfig {
    fn default() -> Self {
        Self {
            trackers: 100,
            tracker_revisions: 30,
            // Default to None to allow all schedules.
            tracker_schedules: None,
            // Default to 10 seconds.
            min_schedule_interval: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SubscriptionWebScrapingConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        let config = SubscriptionWebScrapingConfig::default();
        assert_toml_snapshot!(config, @r###"
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionWebScrapingConfig = toml::from_str(
            r#"
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10_000
    "#,
        )
        .unwrap();
        assert_eq!(config, SubscriptionWebScrapingConfig::default());
    }
}
