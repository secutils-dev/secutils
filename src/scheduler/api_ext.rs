use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    scheduler::{SchedulerJobMetadata, SchedulerJobRetryState, SchedulerJobRetryStrategy},
};
use std::ops::Add;
use time::OffsetDateTime;
use tokio_cron_scheduler::JobId;

pub struct SchedulerApiExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> SchedulerApiExt<'a, DR, ET> {
    /// Creates Scheduler API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Tries to schedule a retry for a specified job. If retry is not possible, returns `None`.
    pub async fn schedule_retry(
        &self,
        job_id: JobId,
        retry_strategy: &SchedulerJobRetryStrategy,
    ) -> anyhow::Result<Option<SchedulerJobRetryState>> {
        let db = &self.api.db;
        let SchedulerJobMetadata { job_type, retry } =
            db.get_scheduler_job_meta(job_id).await?.ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not find a job state for a scheduler job ('{}').",
                    job_id
                )
            })?;

        let retry_attempts = retry
            .map(|retry_state| retry_state.attempts)
            .unwrap_or_default();
        // Check if retry is possible.
        let retry_state = if retry_attempts >= retry_strategy.max_attempts() {
            log::warn!(
                "Retry limit reached ('{}') for a scheduler job ('{job_id}').",
                retry_attempts
            );
            None
        } else {
            let retry_interval = retry_strategy.interval(retry_attempts);
            log::debug!(
                "Scheduling a retry for job ('{job_id}') in {}.",
                humantime::format_duration(retry_interval),
            );

            Some(SchedulerJobRetryState {
                attempts: retry_attempts + 1,
                next_at: OffsetDateTime::now_utc().add(retry_interval),
            })
        };

        db.update_scheduler_job_meta(
            job_id,
            SchedulerJobMetadata {
                job_type,
                retry: retry_state,
            },
        )
        .await?;

        Ok(retry_state)
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with scheduler jobs.
    pub fn scheduler(&self) -> SchedulerApiExt<DR, ET> {
        SchedulerApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        scheduler::{SchedulerJob, SchedulerJobMetadata, SchedulerJobRetryStrategy},
        tests::mock_api,
    };
    use std::{ops::Add, time::Duration};
    use time::OffsetDateTime;
    use tokio_cron_scheduler::{CronJob, JobStored, JobStoredData, JobType};
    use uuid::uuid;

    #[tokio::test]
    async fn properly_schedules_retry() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let scheduler = api.scheduler();

        let job_id = uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");
        let job = JobStoredData {
            id: Some(job_id.into()),
            last_updated: Some(946720800u64),
            last_tick: Some(946720700u64),
            next_tick: 946720900u64,
            count: 3,
            job_type: JobType::Cron as i32,
            extra: SchedulerJobMetadata::new(SchedulerJob::NotificationsSend).try_into()?,
            ran: true,
            stopped: false,
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 0 1 1 * *".to_string(),
            })),
            time_offset_seconds: 0,
        };

        api.db.upsert_scheduler_job(&job).await?;

        let now = OffsetDateTime::now_utc();
        let retry_state = scheduler
            .schedule_retry(
                job_id,
                &SchedulerJobRetryStrategy::Constant {
                    interval: Duration::from_secs(120),
                    max_attempts: 2,
                },
            )
            .await?
            .unwrap();
        assert_eq!(retry_state.attempts, 1);
        assert!(retry_state.next_at >= now.add(Duration::from_secs(120)));

        let retry_state = scheduler
            .schedule_retry(
                job_id,
                &SchedulerJobRetryStrategy::Constant {
                    interval: Duration::from_secs(120),
                    max_attempts: 2,
                },
            )
            .await?
            .unwrap();
        assert_eq!(retry_state.attempts, 2);
        assert!(retry_state.next_at >= now.add(Duration::from_secs(120)));

        let retry_state = scheduler
            .schedule_retry(
                job_id,
                &SchedulerJobRetryStrategy::Constant {
                    interval: Duration::from_secs(120),
                    max_attempts: 2,
                },
            )
            .await?;
        assert!(retry_state.is_none());

        Ok(())
    }
}
