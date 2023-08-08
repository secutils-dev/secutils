mod primary_db_ext;
mod scheduler_job;
mod scheduler_jobs;
mod scheduler_store;

use anyhow::anyhow;
use futures::{pin_mut, StreamExt};
use std::{collections::HashSet, sync::Arc};
use tokio_cron_scheduler::{JobScheduler, SimpleJobCode, SimpleNotificationCode};
use uuid::Uuid;

use crate::{
    api::Api,
    scheduler::{
        scheduler_job::SchedulerJob,
        scheduler_jobs::{ResourcesTrackersDispatchJob, ResourcesTrackersTriggerJob},
    },
};
use scheduler_store::SchedulerStore;

/// Defines a maximum number of jobs that can be retrieved from the database at once.
const MAX_JOBS_PAGE_SIZE: usize = 1000;

/// The scheduler is responsible for scheduling and executing Secutils.dev jobs.
pub struct Scheduler {
    inner_scheduler: JobScheduler,
    api: Arc<Api>,
}

impl Scheduler {
    /// Starts the scheduler resuming existing jobs and adding new ones.
    pub async fn start(api: Api) -> anyhow::Result<Self> {
        let scheduler = Self {
            inner_scheduler: JobScheduler::new_with_storage_and_code(
                Box::new(SchedulerStore::new(api.datastore.primary_db.clone())),
                Box::new(SchedulerStore::new(api.datastore.primary_db.clone())),
                Box::<SimpleJobCode>::default(),
                Box::<SimpleNotificationCode>::default(),
            )
            .await?,
            api: Arc::new(api),
        };

        // First, try to resume existing jobs.
        let resumed_unique_jobs = scheduler.resume().await?;
        if !resumed_unique_jobs.contains(&SchedulerJob::ResourcesTrackersDispatch) {
            scheduler
                .inner_scheduler
                .add(ResourcesTrackersDispatchJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        scheduler.inner_scheduler.start().await?;
        Ok(scheduler)
    }

    /// Resumes existing jobs.
    async fn resume(&self) -> anyhow::Result<HashSet<SchedulerJob>> {
        let db = &self.api.datastore.primary_db;
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

            let job_type = match SchedulerJob::try_from(job_data.extra.as_ref()) {
                Ok(job_type) if unique_resumed_jobs.contains(&job_type) => {
                    // There can only be one job of each type. If we detect that there are multiple, we log
                    // a warning and remove the job, keeping only the first one.
                    log::error!(
                        "Found multiple jobs of type `{:?}`. All duplicated jobs except for the first one will be removed.",
                        job_type
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
                Ok(job_type) => job_type,
            };

            // First try to resume the job, and if it's not possible, the job will be removed and
            // re-scheduled at a later step if needed.
            let job = match &job_type {
                SchedulerJob::ResourcesTrackersTrigger => {
                    ResourcesTrackersTriggerJob::try_resume(self.api.clone(), job_id, job_data)
                        .await?
                }
                SchedulerJob::ResourcesTrackersDispatch => {
                    ResourcesTrackersDispatchJob::try_resume(self.api.clone(), job_id, job_data)
                        .await?
                }
            };

            match job {
                Some(job) => {
                    log::debug!("Resumed job (`{:?}`): {}.", job_type, job_id);
                    self.inner_scheduler.add(job).await?;

                    if job_type.is_unique() {
                        unique_resumed_jobs.insert(job_type);
                    }
                }
                None => {
                    log::warn!(
                        "Failed to resume job (`{:?}`): {}. The job will be removed and re-scheduled if needed.",
                        job_type,
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
    use super::Scheduler;
    use crate::{
        api::Api,
        scheduler::scheduler_job::SchedulerJob,
        tests::{mock_api, mock_user},
        utils::WebPageResourcesTracker,
    };
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use std::time::Duration;
    use tokio_cron_scheduler::{CronJob, JobId, JobStored, JobStoredData, JobType};
    use url::Url;
    use uuid::uuid;

    fn mock_job_data(
        job_id: JobId,
        typ: SchedulerJob,
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
            extra: vec![typ as u8],
            job: Some(JobStored::CronJob(CronJob {
                schedule: schedule.into(),
            })),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_resume_jobs() -> anyhow::Result<()> {
        let user = mock_user();
        let api = mock_api().await?;

        let trigger_job_id = uuid!("00000000-0000-0000-0000-000000000001");
        let dispatch_job_id = uuid!("00000000-0000-0000-0000-000000000002");

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 0,
                    delay: Duration::from_millis(2000),
                    schedule: Some("1 2 3 4 5 6 2030".to_string()),
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", Some(trigger_job_id))
            .await?;

        // Add job registrations.
        api.datastore
            .primary_db
            .upsert_scheduler_job(&mock_job_data(
                trigger_job_id,
                SchedulerJob::ResourcesTrackersTrigger,
                "1 2 3 4 5 6 2030",
            ))
            .await?;
        api.datastore
            .primary_db
            .upsert_scheduler_job(&mock_job_data(
                dispatch_job_id,
                SchedulerJob::ResourcesTrackersDispatch,
                "0 * * * * * *",
            ))
            .await?;

        let mut scheduler = Scheduler::start(Api {
            datastore: api.datastore.clone(),
            config: api.config.clone(),
        })
        .await?;

        assert!(scheduler
            .inner_scheduler
            .next_tick_for_job(trigger_job_id)
            .await?
            .is_some());

        assert!(scheduler
            .inner_scheduler
            .next_tick_for_job(dispatch_job_id)
            .await?
            .is_some());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_unique_jobs_if_not_started() -> anyhow::Result<()> {
        let api = mock_api().await?;

        Scheduler::start(Api {
            datastore: api.datastore.clone(),
            config: api.config.clone(),
        })
        .await?;

        let mut jobs = api
            .datastore
            .primary_db
            .get_scheduler_jobs(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(jobs.len(), 1);

        let dispatch_job = jobs.remove(0)?;
        assert_debug_snapshot!((dispatch_job.job_type, dispatch_job.extra, dispatch_job.job), @r###"
        (
            0,
            [
                1,
            ],
            Some(
                CronJob(
                    CronJob {
                        schedule: "0 * * * * * *",
                    },
                ),
            ),
        )
        "###);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_unique_jobs_if_cannot_resume() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let dispatch_job_id = uuid!("00000000-0000-0000-0000-000000000001");

        // Add job registration.
        api.datastore
            .primary_db
            .upsert_scheduler_job(&mock_job_data(
                dispatch_job_id,
                SchedulerJob::ResourcesTrackersDispatch,
                // Different schedule - every hour, not every minute.
                "0 0 * * * * *",
            ))
            .await?;

        Scheduler::start(Api {
            datastore: api.datastore.clone(),
            config: api.config.clone(),
        })
        .await?;

        let mut jobs = api
            .datastore
            .primary_db
            .get_scheduler_jobs(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(jobs.len(), 1);

        let dispatch_job = jobs.remove(0)?;
        assert_debug_snapshot!((dispatch_job.job_type, dispatch_job.extra, dispatch_job.job), @r###"
        (
            0,
            [
                1,
            ],
            Some(
                CronJob(
                    CronJob {
                        schedule: "0 * * * * * *",
                    },
                ),
            ),
        )
        "###);

        assert_ne!(dispatch_job.id, Some(dispatch_job_id.into()));

        Ok(())
    }
}
