use crate::utils::web_scraping::{PageTrackerConfig, page_trackers::PageTrackerTarget};
use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct PageTrackerUpdateParams {
    /// Arbitrary name of the page tracker.
    pub name: Option<String>,
    /// Page tracker configuration.
    pub config: Option<PageTrackerConfig>,
    /// Page tracker configuration.
    pub target: Option<PageTrackerTarget>,
    /// Indicates whether the user should be notified about changes.
    pub notifications: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::{
        PageTrackerConfig, api_ext::PageTrackerUpdateParams, page_trackers::PageTrackerTarget,
    };
    use retrack_types::scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy};
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PageTrackerUpdateParams>(
                r#"
    {
        "name": "tck"
    }
              "#
            )?,
            PageTrackerUpdateParams {
                name: Some("tck".to_string()),
                ..Default::default()
            }
        );

        assert_eq!(
            serde_json::from_str::<PageTrackerUpdateParams>(
                r#"
    {
        "config": {
            "revisions": 3
        }
    }
              "#
            )?,
            PageTrackerUpdateParams {
                name: None,
                config: Some(PageTrackerConfig {
                    revisions: 3,
                    job: None
                }),
                target: None,
                notifications: false
            }
        );

        assert_eq!(
            serde_json::from_str::<PageTrackerUpdateParams>(
                r#"
    {
        "config": {
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
        }
    }
              "#
            )?,
            PageTrackerUpdateParams {
                name: None,
                config: Some(PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                            initial_interval: Duration::from_millis(1234),
                            multiplier: 2,
                            max_interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    })
                }),
                target: None,
                notifications: false
            }
        );

        assert_eq!(
            serde_json::from_str::<PageTrackerUpdateParams>(
                r#"
    {
        "config": null
    }
              "#
            )?,
            PageTrackerUpdateParams::default()
        );

        assert_eq!(
            serde_json::from_str::<PageTrackerUpdateParams>(
                r#"
    {
        "name": "tck",
        "config": {
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
        },
        "target": {
            "extractor": "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }"
        },
        "notifications": true
    }
              "#
            )?,
            PageTrackerUpdateParams {
                name: Some("tck".to_string()),
                config: Some(PageTrackerConfig {
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
                }),
                target: Some(PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                }),
                notifications: true,
            }
        );

        Ok(())
    }
}
