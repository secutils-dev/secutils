use retrack_types::scheduler::SchedulerJobConfig;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageTrackerConfig {
    /// A number of revisions of the page to track.
    pub revisions: usize,
    /// Configuration for a job, if the tracker needs to be scheduled for automatic change detection.
    pub job: Option<SchedulerJobConfig>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::PageTrackerConfig;
    use retrack_types::scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy};
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let config = PageTrackerConfig {
            revisions: 3,
            job: None,
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerConfig>(&json!({ "revisions": 3 }).to_string())?,
            config
        );

        let config = PageTrackerConfig {
            revisions: 3,
            job: Some(SchedulerJobConfig {
                schedule: "0 0 * * *".to_string(),
                retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                    initial_interval: Duration::from_millis(1234),
                    multiplier: 2,
                    max_interval: Duration::from_secs(120),
                    max_attempts: 5,
                }),
            }),
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerConfig>(
                &json!({
                    "revisions": 3,
                    "job": {
                        "schedule": "0 0 * * *",
                        "retryStrategy": {
                          "type": "exponential",
                          "initialInterval": 1234,
                          "multiplier": 2,
                          "maxInterval": 120000,
                          "maxAttempts": 5
                        }
                    }
                })
                .to_string()
            )?,
            config
        );

        Ok(())
    }
}
