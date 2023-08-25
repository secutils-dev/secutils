use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    scheduler::{scheduler_job::SchedulerJob, scheduler_jobs::ResourcesTrackersTriggerJob},
};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobId, JobScheduler, JobStoredData};

/// The job executes every minute by default to check if there are any trackers to schedule jobs for.
pub(crate) struct ResourcesTrackersScheduleJob;
impl ResourcesTrackersScheduleJob {
    /// Tries to resume existing `ResourcesTrackersSchedule` job.
    pub async fn try_resume<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
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

    /// Creates a new `ResourcesTrackersSchedule` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
    ) -> anyhow::Result<Job> {
        let mut job = Job::new_async(
            api.config.jobs.resources_trackers_schedule.clone(),
            move |_, scheduler| {
                let api = api.clone();
                Box::pin(async move {
                    if let Err(err) = Self::execute(api, scheduler).await {
                        log::error!(
                            "Failed to execute resources trackers schedule job: {:?}",
                            err
                        );
                    }
                })
            },
        )?;

        let job_data = job.job_data()?;
        job.set_job_data(JobStoredData {
            extra: vec![SchedulerJob::ResourcesTrackersSchedule as u8],
            ..job_data
        })?;

        Ok(job)
    }

    /// Executes a `ResourcesTrackersSchedule` job.
    async fn execute<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()> {
        // First check if there any trackers to schedule.
        let web_scraping = api.web_scraping();
        let unscheduled_trackers_jobs = web_scraping
            .get_unscheduled_resources_tracker_jobs()
            .await?;

        log::debug!(
            "Found {} unscheduled trackers jobs.",
            unscheduled_trackers_jobs.len()
        );

        for tracker_job in unscheduled_trackers_jobs {
            let tracker_name = tracker_job.key.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Found an unscheduled tracker job without a tracker name: {:?}",
                    tracker_job
                )
            })?;

            // Check if the tracker still exists, and it supports tracking.
            let tracker = web_scraping
                .get_resources_tracker(tracker_job.user_id, tracker_name)
                .await?;
            let schedule = tracker.and_then(|tracker| {
                if tracker.revisions > 0 {
                    tracker.schedule
                } else {
                    None
                }
            });
            let schedule = if let Some(schedule) = schedule {
                schedule
            } else {
                log::warn!(
                    "Found an unscheduled tracker job for a tracker that doesn't support tracking, removing: {:?}",
                    tracker_job
                );
                web_scraping
                    .remove_resources_tracker_job(tracker_job.user_id, tracker_name)
                    .await?;
                continue;
            };

            // Now, create and schedule a new job.
            let job_id = scheduler
                .add(ResourcesTrackersTriggerJob::create(api.clone(), schedule).await?)
                .await?;
            web_scraping
                .upsert_resources_tracker_job(tracker_job.user_id, tracker_name, Some(job_id))
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ResourcesTrackersScheduleJob;
    use crate::{
        scheduler::{scheduler_job::SchedulerJob, scheduler_store::SchedulerStore},
        tests::{mock_api_with_config, mock_config, mock_user},
        utils::WebPageResourcesTracker,
    };
    use anyhow::anyhow;
    use cron::Schedule;
    use futures::StreamExt;
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
            extra: vec![SchedulerJob::ResourcesTrackersSchedule as u8],
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[actix_rt::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_schedule = Schedule::try_from("1/5 * * * * *")?;

        let api = mock_api_with_config(config).await?;

        let mut job = ResourcesTrackersScheduleJob::create(Arc::new(api)).await?;
        let job_data = job
            .job_data()
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job))?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                1,
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
        config.jobs.resources_trackers_schedule = Schedule::try_from("0 0 * * * *")?;

        let api = mock_api_with_config(config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job =
            ResourcesTrackersScheduleJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id))
                .await?;
        let job_data = job
            .and_then(|mut job| job.job_data().ok())
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job));
        assert_debug_snapshot!(job_data, @r###"
        Some(
            (
                0,
                [
                    1,
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

    #[actix_rt::test]
    async fn resets_job_if_schedule_changed() -> anyhow::Result<()> {
        let api = mock_api_with_config(mock_config()?).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job =
            ResourcesTrackersScheduleJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_schedule_trackers_jobs() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    schedule: Some("1 2 3 4 5 6 2030".to_string()),
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-two".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    schedule: Some("1 2 3 4 5 6 2035".to_string()),
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", None)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-two", None)
            .await?;

        let unscheduled_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert_eq!(unscheduled_jobs.len(), 2);
        assert_eq!(unscheduled_jobs[0].key, Some("tracker-one".to_string()));
        assert_eq!(unscheduled_jobs[1].key, Some("tracker-two".to_string()));

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        let schedule_job_id = scheduler
            .add(ResourcesTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        while !api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }
        scheduler.shutdown().await?;

        // All pending jobs should be scheduled now.
        let unscheduled_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(unscheduled_jobs.is_empty());

        let jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 3);
        for job_data in jobs {
            let job_id = job_data?
                .id
                .ok_or_else(|| anyhow!("Job without ID"))?
                .into();
            if job_id == schedule_job_id {
                continue;
            }

            let job = api
                .web_scraping()
                .get_resources_tracker_job_by_id(job_id)
                .await?;
            assert!(job.is_some());
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_unscheduled_trackers_jobs_if_schedule_removed() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

        // Create user, trackers and tracker jobs.
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
        api.web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-two".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    schedule: None,
                },
            )
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", None)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-two", None)
            .await?;

        let unscheduled_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert_eq!(unscheduled_jobs.len(), 2);
        assert_eq!(unscheduled_jobs[0].key, Some("tracker-one".to_string()));
        assert_eq!(unscheduled_jobs[1].key, Some("tracker-two".to_string()));

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        let schedule_job_id = scheduler
            .add(ResourcesTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        while !api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }
        scheduler.shutdown().await?;

        // All pending jobs should be scheduled now.
        let unscheduled_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(unscheduled_jobs.is_empty());

        let mut jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs.remove(0)?.id, Some(schedule_job_id.into()));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_unscheduled_trackers_jobs_if_revisions_is_zero() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

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
            .upsert_resources_tracker_job(user.id, "tracker-one", None)
            .await?;

        let unscheduled_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert_eq!(unscheduled_jobs.len(), 1);
        assert_eq!(unscheduled_jobs[0].key, Some("tracker-one".to_string()));

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        scheduler
            .add(ResourcesTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        while !api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }
        scheduler.shutdown().await?;

        // All pending jobs should be scheduled now.
        let unscheduled_jobs = api
            .web_scraping()
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(unscheduled_jobs.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_unscheduled_trackers_jobs_if_tracker_do_not_exist() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;

        let web_scraping = api.web_scraping();
        web_scraping
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    schedule: Some("1 2 3 4 5 6 2030".to_string()),
                },
            )
            .await?;
        web_scraping
            .upsert_resources_tracker_job(user.id, "tracker-one", None)
            .await?;
        web_scraping
            .upsert_resources_tracker_job(user.id, "tracker-two", None)
            .await?;

        let unscheduled_jobs = web_scraping
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert_eq!(unscheduled_jobs.len(), 2);
        assert_eq!(unscheduled_jobs[0].key, Some("tracker-one".to_string()));
        assert_eq!(unscheduled_jobs[1].key, Some("tracker-two".to_string()));

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        let schedule_job_id = scheduler
            .add(ResourcesTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        while !web_scraping
            .get_unscheduled_resources_tracker_jobs()
            .await?
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }
        scheduler.shutdown().await?;

        // All pending jobs should be scheduled now.
        let unscheduled_jobs = web_scraping
            .get_unscheduled_resources_tracker_jobs()
            .await?;
        assert!(unscheduled_jobs.is_empty());

        let jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 2);

        for job_data in jobs {
            let job_id = job_data?
                .id
                .ok_or_else(|| anyhow!("Job without ID"))?
                .into();
            if job_id == schedule_job_id {
                continue;
            }

            let job = web_scraping.get_resources_tracker_job_by_id(job_id).await?;
            assert_eq!(job.and_then(|job| job.key), Some("tracker-one".to_string()));
        }

        Ok(())
    }
}
