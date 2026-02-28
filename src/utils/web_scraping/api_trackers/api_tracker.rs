use crate::{
    retrack::RetrackTracker,
    users::{SecretsAccess, UserId},
};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiTracker {
    /// Unique tracker id (UUIDv7).
    pub id: Uuid,
    /// Arbitrary name of the tracker.
    pub name: String,
    /// ID of the user who owns the tracker.
    #[serde(skip_serializing)]
    pub user_id: UserId,
    /// By-value or by-reference instance of the Retrack tracker associated with the current tracker.
    pub retrack: RetrackTracker,
    /// Controls which user secrets are available to this tracker's scripts.
    #[serde(default, skip_serializing_if = "SecretsAccess::is_none")]
    pub secrets: SecretsAccess,
    /// Date and time when the tracker was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    /// Date and time when the tracker was last updated.
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::{
        retrack::{RetrackTracker, tests::RetrackTrackerValue},
        tests::mock_user,
        users::SecretsAccess,
        utils::web_scraping::ApiTracker,
    };
    use insta::assert_json_snapshot;
    use retrack_types::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        trackers::{ApiTarget, TargetRequest, TrackerConfig, TrackerTarget},
    };
    use std::time::Duration;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let tracker = ApiTracker {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "some-name".to_string(),
            user_id: mock_user()?.id,
            retrack: RetrackTracker::Reference {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
            },
            secrets: SecretsAccess::None,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "retrack": {
            "id": "00000000-0000-0000-0000-000000000002"
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "###);

        let tracker = ApiTracker {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "some-name".to_string(),
            user_id: mock_user()?.id,
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: uuid!("00000000-0000-0000-0000-000000000010"),
                enabled: true,
                config: TrackerConfig {
                    revisions: 3,
                    timeout: None,
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: TrackerTarget::Api(ApiTarget {
                    requests: vec![TargetRequest::new(Url::parse(
                        "https://api.example.com/data",
                    )?)],
                    configurator: None,
                    extractor: None,
                    params: None,
                }),
                notifications: false,
            })),
            secrets: SecretsAccess::None,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };

        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "retrack": {
            "id": "00000000-0000-0000-0000-000000000010",
            "enabled": true,
            "config": {
              "revisions": 3,
              "job": {
                "schedule": "@hourly",
                "retryStrategy": {
                  "type": "constant",
                  "interval": 120000,
                  "maxAttempts": 5
                }
              }
            },
            "target": {
              "url": "https://api.example.com/data"
            },
            "notifications": false
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "###);

        Ok(())
    }
}
