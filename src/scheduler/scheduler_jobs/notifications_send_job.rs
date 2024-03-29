use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    scheduler::{
        database_ext::RawSchedulerJobStoredData, job_ext::JobExt, scheduler_job::SchedulerJob,
    },
};
use std::{sync::Arc, time::Instant};
use tokio_cron_scheduler::{Job, JobScheduler};

/// Defines a maximum number of notifications that can be send during a single job tick.
const MAX_NOTIFICATIONS_TO_SEND: usize = 100;

/// The job executes on a regular interval to check if there are any pending notifications that need to be sent.
pub(crate) struct NotificationsSendJob;
impl NotificationsSendJob {
    /// Tries to resume existing `NotificationsSendJob` job.
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

    /// Creates a new `NotificationsSendJob` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
    ) -> anyhow::Result<Job>
    where
        ET::Error: EmailTransportError,
    {
        let mut job = Job::new_async(
            api.config.scheduler.notifications_send.clone(),
            move |_, scheduler| {
                let api = api.clone();
                Box::pin(async move {
                    if let Err(err) = Self::execute(api, scheduler).await {
                        log::error!("Failed to execute notifications send job: {:?}", err);
                    }
                })
            },
        )?;

        job.set_job_type(SchedulerJob::NotificationsSend)?;

        Ok(job)
    }

    /// Executes a `NotificationsSendJob` job.
    async fn execute<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        _: JobScheduler,
    ) -> anyhow::Result<()>
    where
        ET::Error: EmailTransportError,
    {
        let execute_start = Instant::now();
        match api
            .notifications()
            .send_pending_notifications(MAX_NOTIFICATIONS_TO_SEND)
            .await
        {
            Ok(sent_notification_count) if sent_notification_count > 0 => {
                log::info!(
                    "Sent {} notifications ({} elapsed).",
                    sent_notification_count,
                    humantime::format_duration(execute_start.elapsed())
                );
            }
            Ok(_) => {
                log::trace!(
                    "No pending notifications to send ({} elapsed).",
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
        scheduler::scheduler_job::SchedulerJob,
        tests::{
            mock_api_with_config, mock_config, mock_schedule_in_sec, mock_scheduler,
            mock_scheduler_job, mock_user,
        },
    };
    use cron::Schedule;
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::{sync::Arc, time::Duration};
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_create_job_with_correct_parameters(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.scheduler.notifications_send = Schedule::try_from("1/5 * * * * *")?;

        let api = mock_api_with_config(pool, config).await?;

        let mut job = NotificationsSendJob::create(Arc::new(api)).await?;
        let job_data = job
            .job_data()
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job))?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                3,
                0,
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
        config.scheduler.notifications_send = Schedule::try_from("0 0 * * * *")?;

        let api = mock_api_with_config(pool, config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job = NotificationsSendJob::try_resume(
            Arc::new(api),
            mock_scheduler_job(job_id, SchedulerJob::NotificationsSend, "0 0 * * * *"),
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
                    3,
                    0,
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

    #[sqlx::test]
    async fn can_send_pending_notifications(pool: PgPool) -> anyhow::Result<()> {
        let mut scheduler = mock_scheduler(&pool).await?;

        let mut config = mock_config()?;
        config.scheduler.notifications_send = Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(pool, config).await?);
        api.db.upsert_user(&user).await?;

        for n in 0..=(MAX_NOTIFICATIONS_TO_SEND as i64) {
            api.notifications()
                .schedule_notification(
                    NotificationDestination::User(user.id),
                    NotificationContent::Text(format!("message {}", n)),
                    OffsetDateTime::from_unix_timestamp(946720800 + n)?,
                )
                .await?;
        }

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
            tokio::time::sleep(Duration::from_millis(10)).await;
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

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 100);
        assert_debug_snapshot!(messages[0], @r###"
        (
            Envelope {
                forward_path: [
                    Address {
                        serialized: "dev-1@secutils.dev",
                        at_start: 5,
                    },
                ],
                reverse_path: Some(
                    Address {
                        serialized: "dev@secutils.dev",
                        at_start: 3,
                    },
                ),
            },
            "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: dev-1@secutils.dev\r\nSubject: [NO SUBJECT]\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nContent-Transfer-Encoding: 7bit\r\n\r\nmessage 0",
        )
        "###);

        Ok(())
    }
}
