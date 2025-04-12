use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    scheduler::{SchedulerJobMetadata, SchedulerJobRetryState, SchedulerJobRetryStrategy},
};
use std::ops::Add;
use time::OffsetDateTime;
use uuid::Uuid;

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
        job_id: Uuid,
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
        scheduler::{
            SchedulerJob, SchedulerJobMetadata, SchedulerJobRetryStrategy,
            database_ext::RawSchedulerJobStoredData,
        },
        tests::{mock_api, mock_upsert_scheduler_job},
    };
    use sqlx::PgPool;
    use std::{ops::Add, time::Duration};
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn properly_schedules_retry(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let scheduler = api.scheduler();

        let job_id = uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");
        let job = RawSchedulerJobStoredData {
            id: job_id,
            last_updated: Some(946720800),
            last_tick: Some(946720700),
            next_tick: Some(946720900),
            count: Some(3),
            job_type: 3,
            extra: Some(SchedulerJobMetadata::new(SchedulerJob::NotificationsSend).try_into()?),
            ran: Some(true),
            stopped: Some(false),
            schedule: None,
            repeating: None,
            time_offset_seconds: Some(0),
            repeated_every: None,
        };

        mock_upsert_scheduler_job(&api.db, &job).await?;

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
