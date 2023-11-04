use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    scheduler::scheduler_job::SchedulerJob,
};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobId, JobStoredData};

/// The job that is executed for every web resources tracker with automatic tracking enabled. The
/// job doesn't do anything except logging, and updating its internal state. This job is supposed to
/// be as lightweight as possible since we might have thousands of them. There are dedicated
/// schedule and fetch jobs that batch all trackers that need to be scheduled and checked for
/// changes in resources respectively.
pub(crate) struct ResourcesTrackersTriggerJob;
impl ResourcesTrackersTriggerJob {
    /// Tries to resume existing `ResourcesTrackersTrigger` job.
    pub async fn try_resume<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        job_id: JobId,
        existing_job_data: JobStoredData,
    ) -> anyhow::Result<Option<Job>> {
        // First, check if the tracker job exists.
        let web_scraping = api.web_scraping();
        let Some(tracker) = web_scraping.get_resources_tracker_by_job_id(job_id).await? else {
            log::warn!(
                "Tracker job reference doesn't exist, the job ('{job_id}') will be removed."
            );
            return Ok(None);
        };

        // Then, check if the tracker can support revisions.
        if tracker.settings.revisions == 0 {
            log::warn!(
                "Tracker ('{}') no cannot store revisions, the job ('{job_id}') will be removed.",
                tracker.id
            );
            web_scraping
                .update_resources_tracker_job(tracker.id, None)
                .await?;
            return Ok(None);
        };

        // Then, check if the tracker still has a schedule.
        let Some(schedule) = tracker.settings.schedule else {
            log::warn!(
                "Tracker ('{}') no longer has a schedule, the job ('{job_id}') will be removed.",
                tracker.id
            );
            web_scraping
                .update_resources_tracker_job(tracker.id, None)
                .await?;
            return Ok(None);
        };

        // If we changed the job parameters, we need to remove the old job and create a new one.
        let mut new_job = Self::create(api.clone(), schedule).await?;
        Ok(if new_job.job_data()?.job == existing_job_data.job {
            new_job.set_job_data(existing_job_data)?;
            Some(new_job)
        } else {
            web_scraping
                .update_resources_tracker_job(tracker.id, None)
                .await?;
            None
        })
    }

    /// Creates a new `ResourcesTrackersTrigger` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        schedule: impl AsRef<str>,
    ) -> anyhow::Result<Job> {
        // Now, create and schedule new job.
        let mut job = Job::new_async(schedule.as_ref(), move |uuid, _| {
            let db = api.db.clone();
            Box::pin(async move {
                // Mark job as stopped to indicate that it needs processing. Schedule job only picks
                // up stopped jobs, processes them, and then un-stops. Stopped flag is basically
                // serving as a pending processing flag. Eventually we might need to add a separate
                // table for pending jobs.
                if let Err(err) = db.set_scheduler_job_stopped_state(uuid, true).await {
                    log::error!(
                        "Error marking resources tracker trigger job as pending: {}",
                        err
                    );
                } else {
                    log::debug!("Successfully run the job: {}", uuid);
                }
            })
        })?;

        let job_data = job.job_data()?;
        job.set_job_data(JobStoredData {
            extra: vec![SchedulerJob::ResourcesTrackersTrigger as u8],
            ..job_data
        })?;

        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::ResourcesTrackersTriggerJob;
    use crate::{
        scheduler::{scheduler_job::SchedulerJob, scheduler_store::SchedulerStore},
        tests::{mock_api, mock_user},
        utils::{ResourcesCreateParams, WebPageResourcesTracker, WebPageResourcesTrackerSettings},
    };
    use insta::assert_debug_snapshot;
    use std::sync::Arc;
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
            extra: vec![SchedulerJob::ResourcesTrackersTrigger as u8],
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[tokio::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mut job = ResourcesTrackersTriggerJob::create(Arc::new(api), "0 0 * * * *").await?;
        let job_data = job.job_data().map(|job_data| {
            (
                job_data.job_type,
                job_data.extra,
                job_data.job,
                job_data.stopped,
            )
        })?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                0,
            ],
            Some(
                CronJob(
                    CronJob {
                        schedule: "0 0 * * * *",
                    },
                ),
            ),
            false,
        )
        "###);

        Ok(())
    }

    #[tokio::test]
    async fn can_resume_job() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                ResourcesCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 4,
                        schedule: Some("0 0 * * * *".to_string()),
                        delay: Default::default(),
                        scripts: Default::default(),
                        enable_notifications: true,
                    },
                },
            )
            .await?;
        api.web_scraping()
            .update_resources_tracker_job(tracker.id, Some(job_id))
            .await?;

        let mut job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?
                .unwrap();

        let job_data = job
            .job_data()
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job))?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                0,
            ],
            Some(
                CronJob(
                    CronJob {
                        schedule: "0 0 * * * *",
                    },
                ),
            ),
        )
        "###);

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        assert_eq!(
            api.web_scraping()
                .get_resources_tracker_by_job_id(job_id)
                .await?
                .unwrap()
                .id,
            tracker.id
        );

        Ok(())
    }

    #[tokio::test]
    async fn resets_job_if_schedule_changes() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                ResourcesCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 4,
                        schedule: Some("1 0 * * * *".to_string()),
                        delay: Default::default(),
                        scripts: Default::default(),
                        enable_notifications: true,
                    },
                },
            )
            .await?;
        api.web_scraping()
            .update_resources_tracker_job(tracker.id, Some(job_id))
            .await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert_eq!(
            unscheduled_trackers,
            vec![WebPageResourcesTracker {
                job_id: None,
                ..tracker
            }]
        );

        assert!(api
            .web_scraping()
            .get_resources_tracker_by_job_id(job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn removes_job_if_tracker_no_longer_has_schedule() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                ResourcesCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 4,
                        schedule: None,
                        delay: Default::default(),
                        scripts: Default::default(),
                        enable_notifications: true,
                    },
                },
            )
            .await?;
        api.web_scraping()
            .update_resources_tracker_job(tracker.id, Some(job_id))
            .await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        assert!(api
            .web_scraping()
            .get_resources_tracker(user.id, tracker.id)
            .await?
            .unwrap()
            .job_id
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn removes_job_if_tracker_no_longer_has_revisions() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                ResourcesCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 0,
                        schedule: Some("0 0 * * * *".to_string()),
                        delay: Default::default(),
                        scripts: Default::default(),
                        enable_notifications: true,
                    },
                },
            )
            .await?;
        api.web_scraping()
            .update_resources_tracker_job(tracker.id, Some(job_id))
            .await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        assert!(api
            .web_scraping()
            .get_resources_tracker(user.id, tracker.id)
            .await?
            .unwrap()
            .job_id
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn removes_job_if_tracker_no_longer_exists() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        assert!(api
            .web_scraping()
            .get_resources_tracker_by_job_id(job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn marks_job_as_stopped_when_run() -> anyhow::Result<()> {
        let api = Arc::new(mock_api().await?);

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        let trigger_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), "1/1 * * * * *").await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        while !api
            .db
            .get_scheduler_job(trigger_job_id)
            .await?
            .map(|job| job.stopped)
            .unwrap_or_default()
        {}

        scheduler.shutdown().await?;

        Ok(())
    }
}
