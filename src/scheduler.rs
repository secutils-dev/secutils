mod api_ext;
mod database_ext;
mod job_ext;
mod schedule_ext;
mod scheduler_job;
mod scheduler_job_config;
mod scheduler_job_metadata;
mod scheduler_job_retry_state;
mod scheduler_job_retry_strategy;
mod scheduler_jobs;
mod scheduler_store;

use anyhow::anyhow;
use futures::{pin_mut, StreamExt};
use std::{collections::HashSet, sync::Arc};
use tokio_cron_scheduler::{JobScheduler, SimpleJobCode, SimpleNotificationCode};
use uuid::Uuid;

pub use self::{
    schedule_ext::ScheduleExt, scheduler_job::SchedulerJob,
    scheduler_job_config::SchedulerJobConfig, scheduler_job_metadata::SchedulerJobMetadata,
    scheduler_job_retry_state::SchedulerJobRetryState,
    scheduler_job_retry_strategy::SchedulerJobRetryStrategy,
};
use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    scheduler::scheduler_jobs::{
        NotificationsSendJob, WebPageTrackersFetchJob, WebPageTrackersScheduleJob,
        WebPageTrackersTriggerJob,
    },
};
use scheduler_store::SchedulerStore;

/// Defines a maximum number of jobs that can be retrieved from the database at once.
const MAX_JOBS_PAGE_SIZE: usize = 1000;

/// The scheduler is responsible for scheduling and executing Secutils.dev jobs.
pub struct Scheduler<DR: DnsResolver, ET: EmailTransport> {
    inner_scheduler: JobScheduler,
    api: Arc<Api<DR, ET>>,
}

impl<DR: DnsResolver, ET: EmailTransport> Scheduler<DR, ET>
where
    ET::Error: EmailTransportError,
{
    /// Starts the scheduler resuming existing jobs and adding new ones.
    pub async fn start(api: Arc<Api<DR, ET>>) -> anyhow::Result<Self> {
        let scheduler = Self {
            inner_scheduler: JobScheduler::new_with_storage_and_code(
                Box::new(SchedulerStore::new(api.db.clone())),
                Box::new(SchedulerStore::new(api.db.clone())),
                Box::<SimpleJobCode>::default(),
                Box::<SimpleNotificationCode>::default(),
            )
            .await?,
            api,
        };

        // First, try to resume existing jobs.
        let resumed_unique_jobs = scheduler.resume().await?;
        if !resumed_unique_jobs.contains(&SchedulerJob::WebPageTrackersSchedule) {
            scheduler
                .inner_scheduler
                .add(WebPageTrackersScheduleJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        if !resumed_unique_jobs.contains(&SchedulerJob::WebPageTrackersFetch) {
            scheduler
                .inner_scheduler
                .add(WebPageTrackersFetchJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        if !resumed_unique_jobs.contains(&SchedulerJob::NotificationsSend) {
            scheduler
                .inner_scheduler
                .add(NotificationsSendJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        scheduler.inner_scheduler.start().await?;
        Ok(scheduler)
    }

    /// Resumes existing jobs.
    async fn resume(&self) -> anyhow::Result<HashSet<SchedulerJob>> {
        let db = &self.api.db;
        let jobs = db.get_scheduler_jobs(MAX_JOBS_PAGE_SIZE);
        pin_mut!(jobs);

        // Track jobs for the job types that should be scheduled only once.
        let mut unique_resumed_jobs = HashSet::new();
        while let Some(job_data) = jobs.next().await {
            let job_data = job_data?;
            let job_id = job_data
                .id
                .as_ref()
                .map(Uuid::from)
                .ok_or_else(|| anyhow!("The job does not have ID: `{:?}`", job_data))?;

            let job_meta = match SchedulerJobMetadata::try_from(job_data.extra.as_ref()) {
                Ok(job_meta) if unique_resumed_jobs.contains(&job_meta.job_type) => {
                    // There can only be one job of each type. If we detect that there are multiple, we log
                    // a warning and remove the job, keeping only the first one.
                    log::error!(
                        "Found multiple jobs of type `{:?}`. All duplicated jobs except for the first one will be removed.",
                        job_meta.job_type
                    );
                    db.remove_scheduler_job(job_id).await?;
                    continue;
                }
                Err(err) => {
                    // We don't fail here, because we want to gracefully handle the legacy jobs.
                    log::error!(
                        "Failed to deserialize job type for job `{:?}`: {:?}. The job will be removed.",
                        job_data,
                        err
                    );
                    db.remove_scheduler_job(job_id).await?;
                    continue;
                }
                Ok(job_meta) => job_meta,
            };

            // First try to resume the job, and if it's not possible, the job will be removed and
            // re-scheduled at a later step if needed.
            let job = match &job_meta.job_type {
                SchedulerJob::WebPageTrackersTrigger { kind } => {
                    WebPageTrackersTriggerJob::try_resume(self.api.clone(), job_id, job_data, *kind)
                        .await?
                }
                SchedulerJob::WebPageTrackersSchedule => {
                    WebPageTrackersScheduleJob::try_resume(self.api.clone(), job_id, job_data)
                        .await?
                }
                SchedulerJob::WebPageTrackersFetch => {
                    WebPageTrackersFetchJob::try_resume(self.api.clone(), job_id, job_data).await?
                }
                SchedulerJob::NotificationsSend => {
                    NotificationsSendJob::try_resume(self.api.clone(), job_id, job_data).await?
                }
            };

            match job {
                Some(job) => {
                    log::debug!("Resumed job (`{:?}`): {}.", job_meta.job_type, job_id);
                    self.inner_scheduler.add(job).await?;

                    if job_meta.job_type.is_unique() {
                        unique_resumed_jobs.insert(job_meta.job_type);
                    }
                }
                None => {
                    log::warn!(
                        "Failed to resume job (`{:?}`): {}. The job will be removed and re-scheduled if needed.",
                        job_meta.job_type,
                        job_id
                    );
                    db.remove_scheduler_job(job_id).await?;
                }
            }
        }

        Ok(unique_resumed_jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::{Scheduler, SchedulerJobConfig, SchedulerJobMetadata};
    use crate::{
        scheduler::scheduler_job::SchedulerJob,
        tests::{mock_api, mock_user},
        utils::{WebPageTrackerCreateParams, WebPageTrackerKind, WebPageTrackerSettings},
    };
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use std::{sync::Arc, vec::Vec};
    use tokio_cron_scheduler::{CronJob, JobId, JobStored, JobStoredData, JobType};
    use uuid::uuid;

    fn mock_job_data(
        job_id: JobId,
        job_type: SchedulerJob,
        schedule: impl Into<String>,
    ) -> JobStoredData {
        JobStoredData {
            id: Some(job_id.into()),
            job_type: JobType::Cron as i32,
            count: 0,
            last_tick: None,
            next_tick: 12,
            ran: false,
            stopped: false,
            last_updated: None,
            extra: SchedulerJobMetadata::new(job_type).try_into().unwrap(),
            job: Some(JobStored::CronJob(CronJob {
                schedule: schedule.into(),
            })),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_resume_jobs() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let resources_trigger_job_id = uuid!("00000000-0000-0000-0000-000000000001");
        let content_trigger_job_id = uuid!("00000000-0000-0000-0000-000000000002");
        let schedule_job_id = uuid!("00000000-0000-0000-0000-000000000003");
        let notifications_send_job_id = uuid!("00000000-0000-0000-0000-000000000004");

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;
        let resources_tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-one".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "1 2 3 4 5 6 2030".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;
        let content_tracker = api
            .web_scraping()
            .create_content_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-one".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "1 2 3 4 5 6 2030".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(resources_tracker.id, Some(resources_trigger_job_id))
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(content_tracker.id, Some(content_trigger_job_id))
            .await?;

        // Add job registrations.
        api.db
            .upsert_scheduler_job(&mock_job_data(
                resources_trigger_job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
                "1 2 3 4 5 6 2030",
            ))
            .await?;
        api.db
            .upsert_scheduler_job(&mock_job_data(
                content_trigger_job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                },
                "1 2 3 4 5 6 2030",
            ))
            .await?;
        api.db
            .upsert_scheduler_job(&mock_job_data(
                schedule_job_id,
                SchedulerJob::WebPageTrackersSchedule,
                "0 * 0 * * * *",
            ))
            .await?;
        api.db
            .upsert_scheduler_job(&mock_job_data(
                notifications_send_job_id,
                SchedulerJob::NotificationsSend,
                "0 * 2 * * * *",
            ))
            .await?;

        let mut scheduler = Scheduler::start(api.clone()).await?;

        assert!(scheduler
            .inner_scheduler
            .next_tick_for_job(resources_trigger_job_id)
            .await?
            .is_some());

        assert!(scheduler
            .inner_scheduler
            .next_tick_for_job(content_trigger_job_id)
            .await?
            .is_some());

        assert!(scheduler
            .inner_scheduler
            .next_tick_for_job(schedule_job_id)
            .await?
            .is_some());

        assert!(scheduler
            .inner_scheduler
            .next_tick_for_job(notifications_send_job_id)
            .await?
            .is_some());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_unique_jobs_if_not_started() -> anyhow::Result<()> {
        let api = Arc::new(mock_api().await?);
        Scheduler::start(api.clone()).await?;

        let jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 3);

        let mut jobs = jobs
            .into_iter()
            .map(|job_result| job_result.map(|job| (job.job_type, job.extra, job.job)))
            .collect::<anyhow::Result<Vec<(_, _, _)>>>()?;
        jobs.sort_by(|job_a, job_b| job_a.1.cmp(&job_b.1));

        assert_debug_snapshot!(jobs, @r###"
        [
            (
                0,
                [
                    1,
                    0,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 * 0 * * * *",
                        },
                    ),
                ),
            ),
            (
                0,
                [
                    2,
                    0,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 * 1 * * * *",
                        },
                    ),
                ),
            ),
            (
                0,
                [
                    3,
                    0,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 * 2 * * * *",
                        },
                    ),
                ),
            ),
        ]
        "###);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_unique_jobs_if_cannot_resume() -> anyhow::Result<()> {
        let api = Arc::new(mock_api().await?);

        let schedule_job_id = uuid!("00000000-0000-0000-0000-000000000001");
        let fetch_job_id = uuid!("00000000-0000-0000-0000-000000000002");
        let notifications_send_job_id = uuid!("00000000-0000-0000-0000-000000000003");

        // Add job registration.
        api.db
            .upsert_scheduler_job(&mock_job_data(
                schedule_job_id,
                SchedulerJob::WebPageTrackersSchedule,
                // Different schedule - every hour, not every minute.
                "0 0 * * * * *",
            ))
            .await?;
        api.db
            .upsert_scheduler_job(&mock_job_data(
                fetch_job_id,
                SchedulerJob::WebPageTrackersFetch,
                // Different schedule - every day, not every minute.
                "0 0 0 * * * *",
            ))
            .await?;
        api.db
            .upsert_scheduler_job(&mock_job_data(
                notifications_send_job_id,
                SchedulerJob::NotificationsSend,
                // Different schedule - every day, not every minute.
                "0 0 0 * * * *",
            ))
            .await?;

        Scheduler::start(api.clone()).await?;

        // Old jobs should have been removed.
        assert!(api.db.get_scheduler_job(schedule_job_id).await?.is_none());
        assert!(api.db.get_scheduler_job(fetch_job_id).await?.is_none());
        assert!(api
            .db
            .get_scheduler_job(notifications_send_job_id)
            .await?
            .is_none());

        let jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 3);

        let mut jobs = jobs
            .into_iter()
            .map(|job_result| job_result.map(|job| (job.job_type, job.extra, job.job)))
            .collect::<anyhow::Result<Vec<(_, _, _)>>>()?;
        jobs.sort_by(|job_a, job_b| job_a.1.cmp(&job_b.1));

        assert_debug_snapshot!(jobs, @r###"
        [
            (
                0,
                [
                    1,
                    0,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 * 0 * * * *",
                        },
                    ),
                ),
            ),
            (
                0,
                [
                    2,
                    0,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 * 1 * * * *",
                        },
                    ),
                ),
            ),
            (
                0,
                [
                    3,
                    0,
                ],
                Some(
                    CronJob(
                        CronJob {
                            schedule: "0 * 2 * * * *",
                        },
                    ),
                ),
            ),
        ]
        "###);

        Ok(())
    }
}
