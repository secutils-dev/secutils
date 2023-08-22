use crate::{api::Api, network::DnsResolver, scheduler::scheduler_job::SchedulerJob};
use anyhow::anyhow;
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
    pub async fn try_resume<DR: DnsResolver>(
        api: Arc<Api<DR>>,
        job_id: JobId,
        existing_job_data: JobStoredData,
    ) -> anyhow::Result<Option<Job>> {
        // First, check if the tracker job exists.
        let web_scraping = api.web_scraping();
        let tracker_job = if let Some(tracker_job) =
            web_scraping.get_resources_tracker_job_by_id(job_id).await?
        {
            tracker_job
        } else {
            log::debug!(
                "Tracker job reference doesn't exist, the job will be removed: {}",
                job_id
            );
            return Ok(None);
        };

        let tracker_name = tracker_job.key.as_ref().ok_or_else(|| {
            anyhow!(
                "Found a tracker job to schedule without a tracker name: {:?}",
                tracker_job
            )
        })?;

        // Then, check if the tracker still exists and has a schedule.
        let tracker = web_scraping
            .get_resources_tracker(tracker_job.user_id, tracker_name)
            .await?;
        let schedule = if let Some(schedule) = tracker.and_then(|tracker| tracker.schedule) {
            schedule
        } else {
            log::warn!(
                "Found a tracker job for tracker that doesn't exist or doesn't have a schedule, removing...: {:?}",
                tracker_job
            );
            web_scraping
                .remove_resources_tracker_job(tracker_job.user_id, tracker_name)
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
                .upsert_resources_tracker_job(tracker_job.user_id, tracker_name, None)
                .await?;
            None
        })
    }

    /// Creates a new `ResourcesTrackersTrigger` job.
    pub async fn create<DR: DnsResolver>(
        api: Arc<Api<DR>>,
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
        utils::WebPageResourcesTracker,
    };
    use insta::assert_debug_snapshot;
    use std::{sync::Arc, time::Duration};
    use tokio_cron_scheduler::{
        CronJob, JobId, JobScheduler, JobStored, JobStoredData, JobType, SimpleJobCode,
        SimpleNotificationCode, SimpleNotificationStore,
    };
    use url::Url;
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

    #[actix_rt::test]
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

    #[actix_rt::test]
    async fn can_resume_job() -> anyhow::Result<()> {
        let user = mock_user();
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 4,
                    delay: Duration::from_millis(2000),
                    schedule: Some("0 0 * * * *".to_string()),
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker", Some(job_id))
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

        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        assert_eq!(
            api.web_scraping()
                .get_resources_tracker_job_by_id(job_id)
                .await?
                .and_then(|job| job.key),
            Some("tracker".to_string())
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn resets_job_if_schedule_changes() -> anyhow::Result<()> {
        let user = mock_user();
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 4,
                    delay: Duration::from_millis(2000),
                    schedule: Some("1 0 * * * *".to_string()),
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker", Some(job_id))
            .await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert_eq!(pending_jobs.len(), 1);
        assert_eq!(pending_jobs[0].user_id, user.id);
        assert_eq!(pending_jobs[0].key, Some("tracker".to_string()));
        assert!(pending_jobs[0].value.is_none());

        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn removes_job_if_tracker_no_longer_has_schedule() -> anyhow::Result<()> {
        let user = mock_user();
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 4,
                    delay: Duration::from_millis(2000),
                    schedule: None,
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker", Some(job_id))
            .await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn removes_job_if_tracker_no_longer_exists() -> anyhow::Result<()> {
        let user = mock_user();
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker", Some(job_id))
            .await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn does_not_resume_if_tracker_job_no_longer_exists() -> anyhow::Result<()> {
        let user = mock_user();
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;

        let job =
            ResourcesTrackersTriggerJob::try_resume(api.clone(), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(job_id)
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
