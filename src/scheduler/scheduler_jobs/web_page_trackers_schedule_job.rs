use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    scheduler::{
        job_ext::JobExt, scheduler_job::SchedulerJob, scheduler_jobs::WebPageTrackersTriggerJob,
    },
    utils::web_scraping::{WebPageTracker, WebPageTrackerTag},
};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobId, JobScheduler, JobStoredData};

/// The job executes every minute by default to check if there are any trackers to schedule jobs for.
pub(crate) struct WebPageTrackersScheduleJob;
impl WebPageTrackersScheduleJob {
    /// Tries to resume existing `WebPageTrackersSchedule` job.
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

    /// Creates a new `WebPageTrackersSchedule` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
    ) -> anyhow::Result<Job> {
        let mut job = Job::new_async(
            api.config.jobs.web_page_trackers_schedule.clone(),
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

        job.set_job_type(SchedulerJob::WebPageTrackersSchedule)?;

        Ok(job)
    }

    /// Executes a `WebPageTrackersSchedule` job.
    async fn execute<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()> {
        let web_scraping = api.web_scraping();
        Self::schedule_trackers(
            api.clone(),
            &scheduler,
            web_scraping.get_unscheduled_resources_trackers().await?,
        )
        .await?;

        Self::schedule_trackers(
            api.clone(),
            &scheduler,
            web_scraping.get_unscheduled_content_trackers().await?,
        )
        .await?;

        Ok(())
    }

    async fn schedule_trackers<DR: DnsResolver, ET: EmailTransport, Tag: WebPageTrackerTag>(
        api: Arc<Api<DR, ET>>,
        scheduler: &JobScheduler,
        unscheduled_trackers: Vec<WebPageTracker<Tag>>,
    ) -> anyhow::Result<()> {
        if !unscheduled_trackers.is_empty() {
            log::debug!(
                "Found {} unscheduled trackers ({:?}).",
                unscheduled_trackers.len(),
                Tag::KIND
            );
        }

        for tracker in unscheduled_trackers {
            if tracker.settings.revisions == 0 {
                log::error!(
                    "Found an unscheduled tracker ({}) that doesn't support tracking, skipping…",
                    tracker.id
                );
                continue;
            }

            let schedule = if let Some(job_config) = tracker.job_config {
                job_config.schedule
            } else {
                log::error!(
                    "Found an unscheduled tracker ({}) that doesn't have tracking schedule, skipping…",
                    tracker.id
                );
                continue;
            };

            // Now, create and schedule a new job.
            let job_id = scheduler
                .add(WebPageTrackersTriggerJob::create(api.clone(), schedule, Tag::KIND).await?)
                .await?;
            api.web_scraping()
                .update_web_page_tracker_job(tracker.id, Some(job_id))
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WebPageTrackersScheduleJob;
    use crate::{
        scheduler::{
            scheduler_job::SchedulerJob, scheduler_store::SchedulerStore, SchedulerJobConfig,
            SchedulerJobMetadata,
        },
        tests::{mock_api_with_config, mock_config, mock_user},
        utils::web_scraping::{
            tests::WebPageTrackerCreateParams, WebPageTrackerKind, WebPageTrackerSettings,
        },
    };
    use cron::Schedule;
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use std::{sync::Arc, thread, time::Duration};
    use tokio_cron_scheduler::{
        CronJob, JobId, JobScheduler, JobStored, JobStoredData, JobType, SimpleJobCode,
        SimpleNotificationCode, SimpleNotificationStore,
    };
    use url::Url;
    use uuid::{uuid, Uuid};

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
            extra: SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersSchedule)
                .try_into()
                .unwrap(),
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[tokio::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_schedule = Schedule::try_from("1/5 * * * * *")?;

        let api = mock_api_with_config(config).await?;

        let mut job = WebPageTrackersScheduleJob::create(Arc::new(api)).await?;
        let job_data = job
            .job_data()
            .map(|job_data| (job_data.job_type, job_data.extra, job_data.job))?;
        assert_debug_snapshot!(job_data, @r###"
        (
            0,
            [
                1,
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

    #[tokio::test]
    async fn can_resume_job() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_schedule = Schedule::try_from("0 0 * * * *")?;

        let api = mock_api_with_config(config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job =
            WebPageTrackersScheduleJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id))
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

    #[tokio::test]
    async fn resets_job_if_schedule_changed() -> anyhow::Result<()> {
        let api = mock_api_with_config(mock_config()?).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job =
            WebPageTrackersScheduleJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id))
                .await?;
        assert!(job.is_none());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_schedule_trackers_jobs() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;

        let web_scraping = api.web_scraping();
        let tracker_one = web_scraping
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Duration::from_millis(2000),
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

        let tracker_two = web_scraping
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-two".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Duration::from_millis(2000),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "1 2 3 4 5 6 2035".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;

        let tracker_three = web_scraping
            .create_content_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-two".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=3")?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Duration::from_millis(2000),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "1 2 3 4 5 6 2040".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert_eq!(unscheduled_trackers.len(), 2);
        assert_eq!(unscheduled_trackers[0].id, tracker_one.id);
        assert_eq!(unscheduled_trackers[1].id, tracker_two.id);

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_content_trackers()
            .await?;
        assert_eq!(unscheduled_trackers.len(), 1);
        assert_eq!(unscheduled_trackers[0].id, tracker_three.id);

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        scheduler
            .add(WebPageTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        while !(api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?
            .is_empty()
            && api
                .web_scraping()
                .get_unscheduled_content_trackers()
                .await?
                .is_empty())
        {
            thread::sleep(Duration::from_millis(100));
        }
        scheduler.shutdown().await?;

        // All pending jobs should be scheduled now.
        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_content_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        let jobs = api
            .db
            .get_scheduler_jobs(10)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()?;
        assert_eq!(jobs.len(), 4);

        let resources_jobs = jobs
            .iter()
            .filter_map(|job_data| {
                let job_meta = SchedulerJobMetadata::try_from(job_data.extra.as_ref()).unwrap();
                if matches!(
                    job_meta.job_type,
                    SchedulerJob::WebPageTrackersTrigger {
                        kind: WebPageTrackerKind::WebPageResources
                    }
                ) {
                    Some(Uuid::from(job_data.id.as_ref().unwrap()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(resources_jobs.len(), 2);
        for job_id in resources_jobs {
            let scheduled_tracker = web_scraping.get_resources_tracker_by_job_id(job_id).await?;
            assert!(scheduled_tracker.is_some());
        }

        let content_jobs = jobs
            .iter()
            .filter_map(|job_data| {
                let job_meta = SchedulerJobMetadata::try_from(job_data.extra.as_ref()).unwrap();
                if matches!(
                    job_meta.job_type,
                    SchedulerJob::WebPageTrackersTrigger {
                        kind: WebPageTrackerKind::WebPageContent
                    }
                ) {
                    Some(Uuid::from(job_data.id.as_ref().unwrap()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(content_jobs.len(), 1);
        for job_id in content_jobs {
            let scheduled_tracker = web_scraping.get_content_tracker_by_job_id(job_id).await?;
            assert!(scheduled_tracker.is_some());
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn does_not_schedule_trackers_without_schedule() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;

        let resources_tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Duration::from_millis(2000),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: None,
                },
            )
            .await?;

        let content_tracker = api
            .web_scraping()
            .create_content_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageTrackerSettings {
                        revisions: 1,
                        delay: Duration::from_millis(2000),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: None,
                },
            )
            .await?;

        assert!(api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?
            .is_empty());
        assert!(api
            .web_scraping()
            .get_unscheduled_content_trackers()
            .await?
            .is_empty());

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        let schedule_job_id = scheduler
            .add(WebPageTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        thread::sleep(Duration::from_millis(2000));
        scheduler.shutdown().await?;

        // Tracker has not been assigned job ID.
        assert!(api
            .web_scraping()
            .get_resources_tracker(user.id, resources_tracker.id)
            .await?
            .unwrap()
            .job_id
            .is_none());
        assert!(api
            .web_scraping()
            .get_content_tracker(user.id, content_tracker.id)
            .await?
            .unwrap()
            .job_id
            .is_none());

        let mut jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs.remove(0)?.id, Some(schedule_job_id.into()));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn does_not_schedule_trackers_if_revisions_is_zero() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_schedule = Schedule::try_from("1/1 * * * * *")?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);

        // Create user, trackers and tracker jobs.
        api.users().upsert(user.clone()).await?;

        let resources_tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageTrackerSettings {
                        revisions: 0,
                        delay: Duration::from_millis(2000),
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
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageTrackerSettings {
                        revisions: 0,
                        delay: Duration::from_millis(2000),
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

        assert!(api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?
            .is_empty());
        assert!(api
            .web_scraping()
            .get_unscheduled_content_trackers()
            .await?
            .is_empty());

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;
        let schedule_job_id = scheduler
            .add(WebPageTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        thread::sleep(Duration::from_millis(2000));
        scheduler.shutdown().await?;

        // Tracker has not been assigned job ID.
        assert!(api
            .web_scraping()
            .get_resources_tracker(user.id, resources_tracker.id)
            .await?
            .unwrap()
            .job_id
            .is_none());
        assert!(api
            .web_scraping()
            .get_content_tracker(user.id, content_tracker.id)
            .await?
            .unwrap()
            .job_id
            .is_none());

        let mut jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs.remove(0)?.id, Some(schedule_job_id.into()));

        Ok(())
    }
}
