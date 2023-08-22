use crate::{api::Api, network::DnsResolver, scheduler::scheduler_job::SchedulerJob};
use std::{sync::Arc, time::Instant};
use tokio_cron_scheduler::{Job, JobId, JobScheduler, JobStoredData};

/// Defines a maximum number of notifications that can be send during a single job tick.
const MAX_NOTIFICATIONS_TO_SEND: usize = 100;

/// The job executes on a regular interval to check if there are any pending notifications that need to be sent.
pub(crate) struct NotificationsSendJob;
impl NotificationsSendJob {
    /// Tries to resume existing `NotificationsSendJob` job.
    pub async fn try_resume<DR: DnsResolver>(
        api: Arc<Api<DR>>,
        _: JobId,
        existing_job_data: JobStoredData,
    ) -> anyhow::Result<Option<Job>> {
        // If we changed the job parameters, we need to remove the old job and create a new one.
        let mut new_job = Self::create(api).await?;
        Ok(if new_job.job_data()?.job == existing_job_data.job {
            new_job.set_job_data(existing_job_data)?;
            Some(new_job)
        } else {
            None
        })
    }

    /// Creates a new `NotificationsSendJob` job.
    pub async fn create<DR: DnsResolver>(api: Arc<Api<DR>>) -> anyhow::Result<Job> {
        let mut job = Job::new_async(
            api.config.jobs.notifications_send.clone(),
            move |_, scheduler| {
                let api = api.clone();
                Box::pin(async move {
                    if let Err(err) = Self::execute(api, scheduler).await {
                        log::error!("Failed to execute notifications send job: {:?}", err);
                    }
                })
            },
        )?;

        let job_data = job.job_data()?;
        job.set_job_data(JobStoredData {
            extra: vec![SchedulerJob::NotificationsSend as u8],
            ..job_data
        })?;

        Ok(job)
    }

    /// Executes a `NotificationsSendJob` job.
    async fn execute<DR: DnsResolver>(api: Arc<Api<DR>>, _: JobScheduler) -> anyhow::Result<()> {
        let execute_start = Instant::now();
        match api
            .notifications()
            .send_pending_notifications(MAX_NOTIFICATIONS_TO_SEND)
            .await
        {
            Ok(sent_notification_count) => {
                log::info!(
                    "Sent {} notifications ({} elapsed).",
                    sent_notification_count,
                    humantime::format_duration(execute_start.elapsed())
                );
            }
            Err(err) => {
                log::error!(
                    "Failed to send pending notifications ({} elapsed): {:?}",
                    humantime::format_duration(execute_start.elapsed()),
                    err
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{NotificationsSendJob, MAX_NOTIFICATIONS_TO_SEND};
    use crate::{
        notifications::{NotificationContent, NotificationDestination},
        scheduler::{scheduler_job::SchedulerJob, scheduler_store::SchedulerStore},
        tests::{mock_api_with_config, mock_config, mock_schedule_in_sec},
    };
    use cron::Schedule;
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use std::{sync::Arc, thread, time::Duration};
    use time::OffsetDateTime;
    use tokio_cron_scheduler::{
        CronJob, JobId, JobScheduler, JobStored, JobStoredData, JobType, SimpleJobCode,
        SimpleNotificationCode, SimpleNotificationStore,
    };
    use uuid::uuid;

    fn mock_job_data(job_id: JobId) -> JobStoredData {
        JobStoredData {
            id: Some(job_id.into()),
            job_type: JobType::Cron as i32,
            count: 0,
            last_tick: None,
            next_tick: 100500,
            ran: false,
            stopped: false,
            last_updated: None,
            extra: vec![SchedulerJob::NotificationsSend as u8],
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[actix_rt::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.notifications_send = Schedule::try_from("1/5 * * * * *")?;

        let api = mock_api_with_config(config).await?;

        let mut job = NotificationsSendJob::create(Arc::new(api)).await?;
        let job_data = job
            .job_data()
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job))?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                3,
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

    #[actix_rt::test]
    async fn can_resume_job() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.notifications_send = Schedule::try_from("0 0 * * * *")?;

        let api = mock_api_with_config(config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job =
            NotificationsSendJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id)).await?;
        let job_data = job
            .and_then(|mut job| job.job_data().ok())
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job));
        assert_debug_snapshot!(job_data, @r###"
        Some(
            (
                0,
                [
                    3,
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_send_pending_notifications() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.notifications_send = Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

        let api = Arc::new(mock_api_with_config(config).await?);

        for n in 0..=(MAX_NOTIFICATIONS_TO_SEND as i64) {
            api.notifications()
                .schedule_notification(
                    NotificationDestination::User(123.try_into()?),
                    NotificationContent::String(format!("{}", n)),
                    OffsetDateTime::from_unix_timestamp(946720800 + n)?,
                )
                .await?;
        }

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        scheduler
            .add(NotificationsSendJob::create(api.clone()).await?)
            .await?;

        let timestamp = OffsetDateTime::from_unix_timestamp(946730800)?;
        assert_eq!(
            api.db
                .get_notification_ids(timestamp, 10)
                .collect::<Vec<_>>()
                .await
                .len(),
            101
        );

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        while api
            .db
            .get_notification_ids(timestamp, 10)
            .collect::<Vec<_>>()
            .await
            .len()
            > 1
        {
            thread::sleep(Duration::from_millis(10));
        }

        scheduler.shutdown().await?;

        assert_eq!(
            api.db
                .get_notification_ids(timestamp, 10)
                .collect::<Vec<_>>()
                .await
                .len(),
            1
        );

        Ok(())
    }
}
