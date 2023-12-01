use crate::{scheduler::SchedulerJobConfig, utils::WebPageTrackerSettings};
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageTrackerCreateParams {
    /// Arbitrary name of the web page tracker.
    pub name: String,
    /// URL of the web page to track.
    pub url: Url,
    /// Settings of the web page tracker.
    pub settings: WebPageTrackerSettings,
    /// Configuration for a job, if tracker needs to be scheduled for automatic change detection.
    pub job_config: Option<SchedulerJobConfig>,
}

#[cfg(test)]
mod tests {
    use crate::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        utils::{
            web_scraping::api_ext::WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
            WebPageTrackerCreateParams, WebPageTrackerSettings,
        },
    };
    use std::time::Duration;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageTrackerCreateParams>(
                r#"
    {
        "name": "tck",
        "url": "https://secutils.dev",
        "settings": {
            "revisions": 3,
            "delay": 2000
        }
    }
              "#
            )?,
            WebPageTrackerCreateParams {
                name: "tck".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageTrackerCreateParams>(
                r#"
    {
        "name": "tck",
        "url": "https://secutils.dev",
        "settings": {
            "revisions": 3,
            "delay": 2000,
            "scripts": {
                "resourceFilterMap": "return resource;"
            },
            "headers": {
                "cookie": "my-cookie"
            }
        },
        "jobConfig": {
            "schedule": "0 0 * * *",
            "retryStrategy": {
                "type": "exponential",
                "initialInterval": 1234,
                "multiplier": 2,
                "maxInterval": 120000,
                "maxAttempts": 5
            },
            "notifications": true
        }
    }
              "#
            )?,
            WebPageTrackerCreateParams {
                name: "tck".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Some(
                        [(
                            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "return resource;".to_string()
                        )]
                        .iter()
                        .cloned()
                        .collect()
                    ),
                    headers: Some(
                        [("cookie".to_string(), "my-cookie".to_string())]
                            .into_iter()
                            .collect(),
                    )
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * *".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                        initial_interval: Duration::from_millis(1234),
                        multiplier: 2,
                        max_interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: true,
                }),
            }
        );

        Ok(())
    }
}
