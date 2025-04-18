use crate::{
    api::Api,
    logging::UserLogContext,
    network::{DnsResolver, EmailTransport},
    scheduler::{
        database_ext::RawSchedulerJobStoredData, job_ext::JobExt, scheduler_job::SchedulerJob,
        scheduler_jobs::WebPageTrackersTriggerJob,
    },
    utils::web_scraping::{WebPageTracker, WebPageTrackerTag},
};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

/// The job executes every minute by default to check if there are any trackers to schedule jobs for.
pub(crate) struct WebPageTrackersScheduleJob;
impl WebPageTrackersScheduleJob {
    /// Tries to resume existing `WebPageTrackersSchedule` job.
    pub async fn try_resume<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        existing_job_data: RawSchedulerJobStoredData,
    ) -> anyhow::Result<Option<Job>> {
        // If the schedule has changed, remove existing job and create a new one.
        let mut new_job = Self::create(api).await?;
        Ok(if new_job.are_schedules_equal(&existing_job_data)? {
            new_job.set_raw_job_data(existing_job_data)?;
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
            api.config.scheduler.web_page_trackers_schedule.clone(),
            move |_, scheduler| {
                let api = api.clone();
                Box::pin(async move {
                    if let Err(err) = Self::execute(api, scheduler).await {
                        log::error!("Failed to execute resources trackers schedule job: {err:?}");
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
        let web_scraping_system = api.web_scraping_system();
        Self::schedule_trackers(
            api.clone(),
            &scheduler,
            web_scraping_system
                .get_unscheduled_resources_trackers()
                .await?,
        )
        .await?;

        Self::schedule_trackers(
            api.clone(),
            &scheduler,
            web_scraping_system
                .get_unscheduled_content_trackers()
                .await?,
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
                    user:serde = UserLogContext::new(tracker.user_id),
                    util:serde = tracker.log_context();
                    "Found an unscheduled tracker that doesn't support tracking, skipping…"
                );
                continue;
            }

            let schedule = if let Some(job_config) = tracker.job_config {
                job_config.schedule
            } else {
                log::error!(
                    user:serde = UserLogContext::new(tracker.user_id),
                    util:serde = tracker.log_context();
                    "Found an unscheduled tracker that doesn't have tracking schedule, skipping…"
                );
                continue;
            };

            // Now, create and schedule a new job.
            let job_id = scheduler
                .add(WebPageTrackersTriggerJob::create(api.clone(), schedule, Tag::KIND).await?)
                .await?;
            api.web_scraping_system()
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
        scheduler::{SchedulerJobConfig, SchedulerJobMetadata, scheduler_job::SchedulerJob},
        tests::{mock_api_with_config, mock_config, mock_scheduler, mock_scheduler_job, mock_user},
        utils::web_scraping::{
            WebPageTrackerKind, WebPageTrackerSettings, tests::WebPageTrackerCreateParams,
        },
    };
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::{sync::Arc, time::Duration};
    use url::Url;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_create_job_with_correct_parameters(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.scheduler.web_page_trackers_schedule = "1/5 * * * * *".to_string();

        let api = mock_api_with_config(pool, config).await?;

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

    #[sqlx::test]
    async fn can_resume_job(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.scheduler.web_page_trackers_schedule = "0 0 * * * *".to_string();

        let api = mock_api_with_config(pool, config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job = WebPageTrackersScheduleJob::try_resume(
            Arc::new(api),
            mock_scheduler_job(job_id, SchedulerJob::WebPageTrackersSchedule, "0 0 * * * *"),
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

    #[sqlx::test]
    async fn resets_job_if_schedule_changed(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job = WebPageTrackersScheduleJob::try_resume(
            Arc::new(api),
            mock_scheduler_job(job_id, SchedulerJob::WebPageTrackersSchedule, "0 0 * * * *"),
        )
        .await?;
        assert!(job.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_schedule_trackers_jobs(pool: PgPool) -> anyhow::Result<()> {
        let mut scheduler = mock_scheduler(&pool).await?;

        let mut config = mock_config()?;
        config.scheduler.web_page_trackers_schedule = "1/1 * * * * *".to_string();

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(pool, config).await?);

        // Create user, trackers and tracker jobs.
        api.db.upsert_user(user.clone()).await?;

        let web_scraping = api.web_scraping(&user);
        let tracker_one = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "tracker-one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "1 2 3 4 5 6".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let tracker_two = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "tracker-two".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "1 2 3 4 5 6".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let tracker_three = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "tracker-two".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=3")?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "1 2 3 4 5 6".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?;
        assert_eq!(unscheduled_trackers.len(), 2);
        assert_eq!(unscheduled_trackers[0].id, tracker_one.id);
        assert_eq!(unscheduled_trackers[1].id, tracker_two.id);

        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_content_trackers()
            .await?;
        assert_eq!(unscheduled_trackers.len(), 1);
        assert_eq!(unscheduled_trackers[0].id, tracker_three.id);

        scheduler
            .add(WebPageTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        while !(api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?
            .is_empty()
            && api
                .web_scraping_system()
                .get_unscheduled_content_trackers()
                .await?
                .is_empty())
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        scheduler.shutdown().await?;

        // All pending jobs should be scheduled now.
        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        let unscheduled_trackers = api
            .web_scraping_system()
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

        let web_scraping_system = api.web_scraping_system();
        let resources_jobs = jobs
            .iter()
            .filter_map(|job_data| {
                let job_meta =
                    SchedulerJobMetadata::try_from(job_data.extra.as_deref().unwrap()).unwrap();
                if matches!(
                    job_meta.job_type,
                    SchedulerJob::WebPageTrackersTrigger {
                        kind: WebPageTrackerKind::WebPageResources
                    }
                ) {
                    Some(job_data.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(resources_jobs.len(), 2);
        for job_id in resources_jobs {
            let scheduled_tracker = web_scraping_system
                .get_resources_tracker_by_job_id(job_id)
                .await?;
            assert!(scheduled_tracker.is_some());
        }

        let content_jobs = jobs
            .iter()
            .filter_map(|job_data| {
                let job_meta =
                    SchedulerJobMetadata::try_from(job_data.extra.as_deref().unwrap()).unwrap();
                if matches!(
                    job_meta.job_type,
                    SchedulerJob::WebPageTrackersTrigger {
                        kind: WebPageTrackerKind::WebPageContent
                    }
                ) {
                    Some(job_data.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(content_jobs.len(), 1);
        for job_id in content_jobs {
            let scheduled_tracker = web_scraping_system
                .get_content_tracker_by_job_id(job_id)
                .await?;
            assert!(scheduled_tracker.is_some());
        }

        Ok(())
    }

    #[sqlx::test]
    async fn does_not_schedule_trackers_without_schedule(pool: PgPool) -> anyhow::Result<()> {
        let mut scheduler = mock_scheduler(&pool).await?;

        let mut config = mock_config()?;
        config.scheduler.web_page_trackers_schedule = "1/1 * * * * *".to_string();

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(pool, config).await?);

        // Create user, trackers and tracker jobs.
        api.db.upsert_user(user.clone()).await?;

        let resources_tracker = api
            .web_scraping(&user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "tracker-one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: None,
            })
            .await?;

        let content_tracker = api
            .web_scraping(&user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "tracker-one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: None,
            })
            .await?;

        assert!(
            api.web_scraping_system()
                .get_unscheduled_resources_trackers()
                .await?
                .is_empty()
        );
        assert!(
            api.web_scraping_system()
                .get_unscheduled_content_trackers()
                .await?
                .is_empty()
        );

        let schedule_job_id = scheduler
            .add(WebPageTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        tokio::time::sleep(Duration::from_millis(2000)).await;
        scheduler.shutdown().await?;

        // Tracker has not been assigned job ID.
        assert!(
            api.web_scraping(&user)
                .get_resources_tracker(resources_tracker.id)
                .await?
                .unwrap()
                .job_id
                .is_none()
        );
        assert!(
            api.web_scraping(&user)
                .get_content_tracker(content_tracker.id)
                .await?
                .unwrap()
                .job_id
                .is_none()
        );

        let mut jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs.remove(0)?.id, schedule_job_id);

        Ok(())
    }

    #[sqlx::test]
    async fn does_not_schedule_trackers_if_revisions_is_zero(pool: PgPool) -> anyhow::Result<()> {
        let mut scheduler = mock_scheduler(&pool).await?;

        let mut config = mock_config()?;
        config.scheduler.web_page_trackers_schedule = "1/1 * * * * *".to_string();

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(pool, config).await?);

        // Create user, trackers and tracker jobs.
        api.db.upsert_user(user.clone()).await?;

        let resources_tracker = api
            .web_scraping(&user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "tracker-one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 0,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "1 2 3 4 5 6".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;
        let content_tracker = api
            .web_scraping(&user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "tracker-one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 0,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "1 2 3 4 5 6".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        assert!(
            api.web_scraping_system()
                .get_unscheduled_resources_trackers()
                .await?
                .is_empty()
        );
        assert!(
            api.web_scraping_system()
                .get_unscheduled_content_trackers()
                .await?
                .is_empty()
        );

        let schedule_job_id = scheduler
            .add(WebPageTrackersScheduleJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;
        tokio::time::sleep(Duration::from_millis(2000)).await;
        scheduler.shutdown().await?;

        // Tracker has not been assigned job ID.
        assert!(
            api.web_scraping(&user)
                .get_resources_tracker(resources_tracker.id)
                .await?
                .unwrap()
                .job_id
                .is_none()
        );
        assert!(
            api.web_scraping(&user)
                .get_content_tracker(content_tracker.id)
                .await?
                .unwrap()
                .job_id
                .is_none()
        );

        let mut jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs.remove(0)?.id, schedule_job_id);

        Ok(())
    }
}
