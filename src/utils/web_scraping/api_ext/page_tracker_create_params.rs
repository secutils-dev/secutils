use crate::{
    users::SecretsAccess,
    utils::web_scraping::{PageTrackerConfig, page_trackers::PageTrackerTarget},
};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageTrackerCreateParams {
    /// Arbitrary name of the page tracker.
    pub name: String,
    /// Whether the tracker is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Page tracker configuration.
    pub config: PageTrackerConfig,
    /// Page tracker configuration.
    pub target: PageTrackerTarget,
    /// Indicates whether the user should be notified about changes.
    #[serde(default)]
    pub notifications: bool,
    /// Controls which user secrets are available to this tracker's extractor script.
    #[serde(default)]
    pub secrets: SecretsAccess,
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use crate::{
        users::SecretsAccess,
        utils::web_scraping::{
            PageTrackerConfig, api_ext::PageTrackerCreateParams, page_trackers::PageTrackerTarget,
        },
    };
    use retrack_types::scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy};
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PageTrackerCreateParams>(
                r#"
    {
        "name": "tck",
        "config": {
            "revisions": 3
        },
        "target": {
            "extractor": "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }"
        }
    }
              "#
            )?,
            PageTrackerCreateParams {
                name: "tck".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: SecretsAccess::None,
            }
        );

        assert_eq!(
            serde_json::from_str::<PageTrackerCreateParams>(
                r#"
    {
        "name": "tck",
        "enabled": false,
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
            PageTrackerCreateParams {
                name: "tck".to_string(),
                enabled: false,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                            initial_interval: Duration::from_millis(1234),
                            multiplier: 2,
                            max_interval: Duration::from_secs(120),
                            max_attempts: 5,
                        })
                    })
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: true,
                secrets: SecretsAccess::None,
            }
        );

        Ok(())
    }
}
