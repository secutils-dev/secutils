mod api_ext;
mod api_trackers;
mod database_ext;
mod page_trackers;
mod schedule;
mod tracker_kind;

pub use self::{
    api_ext::{
        ApiTrackerCreateParams, ApiTrackerDebugParams, ApiTrackerGetHistoryParams,
        ApiTrackerTestParams, ApiTrackerTestResult, ApiTrackerUpdateParams,
        PageTrackerCreateParams, PageTrackerDebugParams, PageTrackerGetHistoryParams,
        PageTrackerUpdateParams,
    },
    api_trackers::{ApiTracker, ApiTrackerConfig, ApiTrackerTarget},
    page_trackers::{PageTracker, PageTrackerConfig, PageTrackerTarget},
    schedule::{expand_job_config, expand_schedule_preset},
    tracker_kind::TrackerKind,
};

#[cfg(test)]
pub mod tests {
    use crate::{
        retrack::RetrackTracker,
        tests::mock_user,
        users::SecretsAccess,
        utils::web_scraping::{ApiTracker, PageTracker},
    };
    use retrack_types::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        trackers::{ApiTarget, TargetRequest, Tracker, TrackerConfig, TrackerTarget},
    };
    use std::time::Duration;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::{Uuid, uuid};

    pub fn mock_retrack_api_tracker() -> anyhow::Result<Tracker> {
        Ok(Tracker {
            id: uuid!("00000000-0000-0000-0000-000000000010"),
            name: "api_one".to_string(),
            enabled: true,
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest::new(Url::parse(
                    "https://api.example.com/data",
                )?)],
                configurator: None,
                extractor: None,
                params: None,
            }),
            job_id: None,
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
            tags: vec![],
            actions: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            scheduled_at: None,
            last_ran_at: None,
        })
    }

    pub struct MockPageTrackerBuilder {
        tracker: PageTracker,
    }

    impl MockPageTrackerBuilder {
        pub fn create<N: Into<String>>(
            id: Uuid,
            name: N,
            retrack: RetrackTracker,
        ) -> anyhow::Result<Self> {
            Ok(Self {
                tracker: PageTracker {
                    id,
                    name: name.into(),
                    user_id: mock_user()?.id,
                    retrack,
                    secrets: SecretsAccess::None,
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            })
        }

        pub fn with_tag_ids(mut self, tag_ids: &[Uuid]) -> Self {
            self.tracker.tags = tag_ids
                .iter()
                .map(|id| crate::users::EntityTag::from(*id))
                .collect();
            self
        }

        pub fn build(self) -> PageTracker {
            self.tracker
        }
    }

    pub struct MockApiTrackerBuilder {
        tracker: ApiTracker,
    }

    impl MockApiTrackerBuilder {
        pub fn create<N: Into<String>>(
            id: Uuid,
            name: N,
            retrack: RetrackTracker,
        ) -> anyhow::Result<Self> {
            Ok(Self {
                tracker: ApiTracker {
                    id,
                    name: name.into(),
                    user_id: mock_user()?.id,
                    retrack,
                    secrets: SecretsAccess::None,
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            })
        }

        pub fn with_tag_ids(mut self, tag_ids: &[Uuid]) -> Self {
            self.tracker.tags = tag_ids
                .iter()
                .map(|id| crate::users::EntityTag::from(*id))
                .collect();
            self
        }

        pub fn build(self) -> ApiTracker {
            self.tracker
        }
    }
}
