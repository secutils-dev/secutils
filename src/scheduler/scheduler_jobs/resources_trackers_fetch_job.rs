use crate::{
    api::Api, network::DnsResolver, scheduler::scheduler_job::SchedulerJob, users::UserData,
};
use futures::{pin_mut, StreamExt};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobId, JobScheduler, JobStoredData};

/// The job executes every minute by default to check if there are any trackers to fetch resources for.
pub(crate) struct ResourcesTrackersFetchJob;
impl ResourcesTrackersFetchJob {
    /// Tries to resume existing `ResourcesTrackersFetch` job.
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

    /// Creates a new `ResourcesTrackersFetch` job.
    pub async fn create<DR: DnsResolver>(api: Arc<Api<DR>>) -> anyhow::Result<Job> {
        let mut job = Job::new_async(
            api.config.jobs.resources_trackers_fetch.clone(),
            move |_, scheduler| {
                let api = api.clone();
                Box::pin(async move {
                    if let Err(err) = Self::execute(api, scheduler).await {
                        log::error!("Failed to execute resources trackers fetch job: {:?}", err);
                    }
                })
            },
        )?;

        let job_data = job.job_data()?;
        job.set_job_data(JobStoredData {
            extra: vec![SchedulerJob::ResourcesTrackersFetch as u8],
            ..job_data
        })?;

        Ok(job)
    }

    /// Executes a `ResourcesTrackersFetch` job.
    async fn execute<DR: DnsResolver>(
        api: Arc<Api<DR>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()> {
        // Fetch all resources trackers jobs that are pending processing.
        let web_scraping = api.web_scraping();
        let pending_trackers_jobs = web_scraping.get_pending_resources_tracker_jobs();
        pin_mut!(pending_trackers_jobs);

        while let Some(tracker_job) = pending_trackers_jobs.next().await {
            let UserData {
                key: tracker_name,
                user_id,
                value: tracker_job_id,
                ..
            } = tracker_job?;
            let tracker_name = tracker_name.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Found an pending tracker job without a tracker name: {:?}",
                    tracker_job_id
                )
            })?;

            let tracker = web_scraping
                .get_resources_tracker(user_id, tracker_name)
                .await?
                .and_then(|tracker| {
                    if tracker.revisions > 0 && tracker.schedule.is_some() {
                        Some(tracker)
                    } else {
                        None
                    }
                });
            let tracker = if let Some(tracker) = tracker {
                tracker
            } else {
                log::warn!(
                    "Found an pending tracker job for a tracker that doesn't support tracking, removing: {:?} (User ID: {:?})",
                    tracker_name, user_id
                );
                scheduler.remove(&tracker_job_id).await?;
                web_scraping
                    .remove_resources_tracker_job(user_id, tracker_name)
                    .await?;
                continue;
            };

            web_scraping
                .save_resources(
                    user_id,
                    &tracker,
                    web_scraping.fetch_resources(&tracker).await?,
                )
                .await?;

            api.db
                .set_scheduler_job_stopped_state(tracker_job_id, false)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ResourcesTrackersFetchJob;
    use crate::{
        scheduler::{
            scheduler_job::SchedulerJob, scheduler_jobs::ResourcesTrackersTriggerJob,
            scheduler_store::SchedulerStore,
        },
        tests::{mock_api_with_config, mock_config, mock_user},
        utils::WebPageResourcesTracker,
    };
    use cron::Schedule;
    use insta::assert_debug_snapshot;
    use std::{sync::Arc, thread, time::Duration};
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
            extra: vec![SchedulerJob::ResourcesTrackersFetch as u8],
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[actix_rt::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch = Schedule::try_from("1/5 * * * * *")?;

        let api = mock_api_with_config(config).await?;

        let mut job = ResourcesTrackersFetchJob::create(Arc::new(api)).await?;
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

    #[actix_rt::test]
    async fn can_resume_job() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch = Schedule::try_from("0 0 * * * *")?;

        let api = mock_api_with_config(config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job =
            ResourcesTrackersFetchJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id))
                .await?;
        let job_data = job
            .and_then(|mut job| job.job_data().ok())
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job));
        assert_debug_snapshot!(job_data, @r###"
        Some(
            (
                0,
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_pending_trackers_jobs_if_tracker_does_not_exist() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch = Schedule::try_from("1/3 * * * * *")?;

        let user = mock_user();
        let api = Arc::new(mock_api_with_config(config).await?);
        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), "1/1 * * * * *").await?)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", Some(tracker_job_id))
            .await?;

        // Schedule fetch job
        scheduler
            .add(ResourcesTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        thread::sleep(Duration::from_secs(5));
        scheduler.shutdown().await?;

        // There shouldn't be a unscheduled tracker jobs.
        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        // There shouldn't be a tracker job anymore.
        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(tracker_job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_pending_trackers_jobs_if_schedule_removed() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch = Schedule::try_from("1/3 * * * * *")?;

        let user = mock_user();
        let api = Arc::new(mock_api_with_config(config).await?);
        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    schedule: None,
                },
            )
            .await?;
        let tracker_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), "1/1 * * * * *").await?)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", Some(tracker_job_id))
            .await?;

        // Schedule fetch job
        scheduler
            .add(ResourcesTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        thread::sleep(Duration::from_secs(5));
        scheduler.shutdown().await?;

        // There shouldn't be a unscheduled tracker jobs.
        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        // There shouldn't be a tracker job anymore.
        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(tracker_job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_pending_trackers_jobs_if_zero_revisions() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch = Schedule::try_from("1/3 * * * * *")?;

        let user = mock_user();
        let api = Arc::new(mock_api_with_config(config).await?);
        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 0,
                    delay: Duration::from_millis(2000),
                    schedule: Some("1/1 * * * * *".to_string()),
                },
            )
            .await?;
        let tracker_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), "1/1 * * * * *").await?)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", Some(tracker_job_id))
            .await?;

        // Schedule fetch job
        scheduler
            .add(ResourcesTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        thread::sleep(Duration::from_secs(5));
        scheduler.shutdown().await?;

        // There shouldn't be a unscheduled tracker jobs.
        let pending_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(pending_jobs.is_empty());

        // There shouldn't be a tracker job anymore.
        assert!(api
            .web_scraping()
            .get_resources_tracker_job_by_id(tracker_job_id)
            .await?
            .is_none());

        Ok(())
    }
}
