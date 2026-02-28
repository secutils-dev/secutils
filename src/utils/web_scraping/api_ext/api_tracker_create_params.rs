use crate::{
    users::SecretsAccess,
    utils::web_scraping::{ApiTrackerConfig, api_trackers::ApiTrackerTarget},
};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiTrackerCreateParams {
    /// Arbitrary name of the API tracker.
    pub name: String,
    /// Whether the tracker is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// API tracker configuration.
    pub config: ApiTrackerConfig,
    /// API tracker target (single HTTP request definition).
    pub target: ApiTrackerTarget,
    /// Indicates whether the user should be notified about changes.
    #[serde(default)]
    pub notifications: bool,
    /// Controls which user secrets are available to this tracker's scripts.
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
            ApiTrackerConfig, api_ext::ApiTrackerCreateParams, api_trackers::ApiTrackerTarget,
        },
    };
    use retrack_types::scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy};
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ApiTrackerCreateParams>(
                r#"
    {
        "name": "tck",
        "config": { "revisions": 3 },
        "target": { "url": "https://api.example.com/data" }
    }
              "#
            )?,
            ApiTrackerCreateParams {
                name: "tck".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: false,
                secrets: SecretsAccess::None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ApiTrackerCreateParams>(
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
            "url": "https://api.example.com/data",
            "method": "POST",
            "headers": { "Authorization": "Bearer tok" },
            "body": { "key": "value" }
        },
        "notifications": true,
        "secrets": { "type": "all" }
    }
              "#
            )?,
            ApiTrackerCreateParams {
                name: "tck".to_string(),
                enabled: false,
                config: ApiTrackerConfig {
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
                target: ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: Some("POST".to_string()),
                    headers: Some(
                        [("Authorization".to_string(), "Bearer tok".to_string())]
                            .into_iter()
                            .collect()
                    ),
                    body: Some(serde_json::json!({ "key": "value" })),
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: true,
                secrets: SecretsAccess::All,
            }
        );

        Ok(())
    }
}
