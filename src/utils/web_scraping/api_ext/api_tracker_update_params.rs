use crate::{
    users::SecretsAccess,
    utils::web_scraping::{ApiTrackerConfig, api_trackers::ApiTrackerTarget},
};
use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ApiTrackerUpdateParams {
    /// Arbitrary name of the API tracker.
    pub name: Option<String>,
    /// Whether the tracker is enabled.
    pub enabled: Option<bool>,
    /// API tracker configuration.
    pub config: Option<ApiTrackerConfig>,
    /// API tracker target (single HTTP request definition).
    pub target: Option<ApiTrackerTarget>,
    /// Indicates whether the user should be notified about changes.
    pub notifications: bool,
    /// Controls which user secrets are available to this tracker's scripts.
    pub secrets: Option<SecretsAccess>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::{
        ApiTrackerConfig, api_ext::ApiTrackerUpdateParams, api_trackers::ApiTrackerTarget,
    };
    use retrack_types::scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy};
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ApiTrackerUpdateParams>(r#"{ "name": "tck" }"#)?,
            ApiTrackerUpdateParams {
                name: Some("tck".to_string()),
                enabled: None,
                config: None,
                target: None,
                notifications: false,
                secrets: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ApiTrackerUpdateParams>(r#"{ "enabled": false }"#)?,
            ApiTrackerUpdateParams {
                name: None,
                enabled: Some(false),
                config: None,
                target: None,
                notifications: false,
                secrets: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ApiTrackerUpdateParams>(r#"{ "config": { "revisions": 3 } }"#)?,
            ApiTrackerUpdateParams {
                name: None,
                enabled: None,
                config: Some(ApiTrackerConfig {
                    revisions: 3,
                    job: None
                }),
                target: None,
                notifications: false,
                secrets: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ApiTrackerUpdateParams>(r#"{ "config": null }"#)?,
            ApiTrackerUpdateParams::default()
        );

        assert_eq!(
            serde_json::from_str::<ApiTrackerUpdateParams>(
                r#"
    {
        "name": "tck",
        "enabled": true,
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
            "url": "https://api.example.com/data",
            "method": "POST"
        },
        "notifications": true
    }
              "#
            )?,
            ApiTrackerUpdateParams {
                name: Some("tck".to_string()),
                enabled: Some(true),
                config: Some(ApiTrackerConfig {
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
                target: Some(ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: Some("POST".to_string()),
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                }),
                notifications: true,
                secrets: None,
            }
        );

        Ok(())
    }
}
