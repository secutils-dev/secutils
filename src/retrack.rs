mod api_ext;
mod retrack_tracker;

pub mod tags;

pub use self::retrack_tracker::RetrackTracker;

#[cfg(test)]
pub mod tests {
    pub use super::retrack_tracker::RetrackTrackerValue;
    use retrack_types::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        trackers::{PageTarget, Tracker, TrackerConfig, TrackerTarget},
    };
    use std::time::Duration;
    use time::OffsetDateTime;
    use uuid::uuid;

    pub fn mock_retrack_tracker() -> anyhow::Result<Tracker> {
        Ok(Tracker {
            id: uuid!("00000000-0000-0000-0000-000000000010"),
            name: "name_one".to_string(),
            enabled: true,
            target: TrackerTarget::Page(PageTarget {
                extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                params: None,
                engine: None,
                user_agent: None,
                accept_invalid_certificates: false,
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
        })
    }
}
