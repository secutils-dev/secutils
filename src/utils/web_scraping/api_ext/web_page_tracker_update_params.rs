use crate::{scheduler::SchedulerJobConfig, utils::WebPageTrackerSettings};
use serde::{Deserialize, Deserializer};
use url::Url;

#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct WebPageTrackerUpdateParams {
    /// Arbitrary name of the web page tracker.
    pub name: Option<String>,
    /// URL of the web page to track.
    pub url: Option<Url>,
    /// Settings of the web page tracker.
    pub settings: Option<WebPageTrackerSettings>,
    /// Configuration for a job, if tracker needs to be scheduled for automatic change detection.
    /// We use nested `Option` to distinguish between `null` and `undefined` values.
    #[serde(deserialize_with = "deserialize_optional_field")]
    pub job_config: Option<Option<SchedulerJobConfig>>,
}

fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[cfg(test)]
mod tests {
    use crate::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        utils::{
            WebPageTrackerSettings, WebPageTrackerUpdateParams,
            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
        },
    };
    use std::time::Duration;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageTrackerUpdateParams>(
                r#"
    {
        "name": "tck"
    }
              "#
            )?,
            WebPageTrackerUpdateParams {
                name: Some("tck".to_string()),
                url: None,
                settings: None,
                job_config: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageTrackerUpdateParams>(
                r#"
    {
        "settings": {
            "revisions": 3,
            "delay": 2000,
            "scripts": {
                "resourceFilterMap": "return resource;"
            },
            "headers": {
                "cookie": "my-cookie"
            }
        }
    }
              "#
            )?,
            WebPageTrackerUpdateParams {
                name: None,
                url: None,
                settings: Some(WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Some(
                        [(
                            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "return resource;".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    headers: Some(
                        [("cookie".to_string(), "my-cookie".to_string())]
                            .into_iter()
                            .collect(),
                    )
                }),
                job_config: None
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageTrackerUpdateParams>(
                r#"
    {
        "jobConfig": null
    }
              "#
            )?,
            WebPageTrackerUpdateParams {
                name: None,
                url: None,
                settings: None,
                job_config: Some(None)
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageTrackerUpdateParams>(
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
            WebPageTrackerUpdateParams {
                name: Some("tck".to_string()),
                url: Some(Url::parse("https://secutils.dev")?),
                settings: Some(WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Some(
                        [(
                            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "return resource;".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    headers: Some(
                        [("cookie".to_string(), "my-cookie".to_string())]
                            .into_iter()
                            .collect(),
                    )
                }),
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "0 0 * * *".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                        initial_interval: Duration::from_millis(1234),
                        multiplier: 2,
                        max_interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: true,
                })),
            }
        );

        Ok(())
    }
}
