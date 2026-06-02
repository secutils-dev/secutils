use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    scheduler::{
        database_ext::RawSchedulerJobStoredData, job_ext::JobExt, scheduler_job::SchedulerJob,
    },
};
use std::{sync::Arc, time::Instant};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, trace};

/// The job runs on a regular interval to schedule coalesced "responder was hit" notifications for
/// responders that opted in, respecting their configured throttle window.
pub(crate) struct RespondersNotifyJob;
impl RespondersNotifyJob {
    /// Tries to resume existing `RespondersNotifyJob` job.
    pub async fn try_resume<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        existing_job_data: RawSchedulerJobStoredData,
    ) -> anyhow::Result<Option<Job>>
    where
        ET::Error: EmailTransportError,
    {
        // If the schedule has changed, remove existing job and create a new one.
        let mut new_job = Self::create(api).await?;
        Ok(if new_job.are_schedules_equal(&existing_job_data)? {
            new_job.set_raw_job_data(existing_job_data)?;
            Some(new_job)
        } else {
            None
        })
    }

    /// Creates a new `RespondersNotifyJob` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
    ) -> anyhow::Result<Job>
    where
        ET::Error: EmailTransportError,
    {
        let mut job = Job::new_async(
            api.config.scheduler.responders_notify.clone(),
            move |_, scheduler| {
                let api = api.clone();
                Box::pin(async move {
                    if let Err(err) = Self::execute(api, scheduler).await {
                        error!("Failed to execute responders notify job: {err:?}");
                    }
                })
            },
        )?;

        job.set_job_type(SchedulerJob::RespondersNotify)?;

        Ok(job)
    }

    /// Executes a `RespondersNotifyJob` job.
    async fn execute<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        _: JobScheduler,
    ) -> anyhow::Result<()>
    where
        ET::Error: EmailTransportError,
    {
        let execute_start = Instant::now();
        match api.notify_pending_responders().await {
            Ok(scheduled) if scheduled > 0 => {
                info!(
                    "Scheduled {scheduled} responder notifications ({} elapsed).",
                    humantime::format_duration(execute_start.elapsed())
                );
            }
            Ok(_) => {
                trace!(
                    "No responder notifications to schedule ({} elapsed).",
                    humantime::format_duration(execute_start.elapsed())
                );
            }
            Err(err) => {
                error!(
                    "Failed to schedule responder notifications ({} elapsed): {err:?}",
                    humantime::format_duration(execute_start.elapsed())
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RespondersNotifyJob;
    use crate::{
        scheduler::scheduler_job::SchedulerJob,
        tests::{mock_api_with_config, mock_config, mock_scheduler_job},
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::sync::Arc;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_create_job_with_correct_parameters(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.scheduler.responders_notify = "1/5 * * * * *".to_string();

        let api = mock_api_with_config(pool, config).await?;

        let mut job = RespondersNotifyJob::create(Arc::new(api)).await?;
        let job_data = job
            .job_data()
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job))?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                2,
            ],
            Some(
                CronJob(
                    CronJob {
                        schedule: "1/5 * * * * *",
                    },
                ),
            ),
        )
        "###);

        Ok(())
    }

    #[sqlx::test]
    async fn can_resume_job(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.scheduler.responders_notify = "0 0 * * * *".to_string();

        let api = mock_api_with_config(pool, config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job = RespondersNotifyJob::try_resume(
            Arc::new(api),
            mock_scheduler_job(job_id, SchedulerJob::RespondersNotify, "0 0 * * * *"),
        )
        .await?;
        let job_data = job
            .and_then(|mut job| job.job_data().ok())
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job));
        assert_debug_snapshot!(job_data, @r###"
        Some(
            (
                3,
                [
                    2,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 * * * *",
                        },
                    ),
                ),
            ),
        )
        "###);

        Ok(())
    }
}
