use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    notifications::{NotificationContent, NotificationContentTemplate, NotificationDestination},
    scheduler::scheduler_job::SchedulerJob,
};
use futures::{pin_mut, StreamExt};
use std::{sync::Arc, time::Instant};
use time::{Duration, OffsetDateTime};
use tokio_cron_scheduler::{Job, JobId, JobScheduler, JobStoredData};

// If job execution takes more than 10 seconds, we'll log a warning instead of trace/debug message.
const JOB_EXECUTION_THRESHOLD: Duration = Duration::new(10, 0);

/// The job executes every minute by default to check if there are any trackers to fetch resources for.
pub(crate) struct ResourcesTrackersFetchJob;
impl ResourcesTrackersFetchJob {
    /// Tries to resume existing `ResourcesTrackersFetch` job.
    pub async fn try_resume<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        _: JobId,
        existing_job_data: JobStoredData,
    ) -> anyhow::Result<Option<Job>>
    where
        ET::Error: EmailTransportError,
    {
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
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
    ) -> anyhow::Result<Job>
    where
        ET::Error: EmailTransportError,
    {
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
    async fn execute<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()>
    where
        ET::Error: EmailTransportError,
    {
        let execute_start = Instant::now();

        // Fetch all resources trackers jobs that are pending processing.
        let web_scraping = api.web_scraping();
        let pending_trackers = web_scraping.get_pending_resources_trackers();
        pin_mut!(pending_trackers);

        while let Some(tracker) = pending_trackers.next().await {
            let tracker = tracker?;
            let Some(job_id) = tracker.job_id else {
                log::error!(
                    "Could not find a job for a pending resources tracker ('{}'), skipping.",
                    tracker.id
                );
                continue;
            };

            if tracker.settings.revisions == 0 || tracker.settings.schedule.is_none() {
                log::warn!(
                    "Found a pending resources tracker ('{}') that doesn't support tracking, the job ('{:?}') will be removed.",
                    tracker.id, tracker.job_id
                );
                scheduler.remove(&job_id).await?;
                web_scraping
                    .update_resources_tracker_job(tracker.id, None)
                    .await?;
                continue;
            };

            // Check if resources has changed, comparing new revision to the latest existing one.
            let fetch_start = Instant::now();

            // Create a new revision and retrieve a diff if any changes from the previous version are
            // detected. If there are any changes and the tracker hasn't opted out of notifications,
            // schedule a notification about the detected changes.
            let new_revision_with_diff = match web_scraping
                .create_resources_tracker_revision(tracker.user_id, &tracker)
                .await
            {
                Ok(new_revision_with_diff) => new_revision_with_diff,
                Err(err) => {
                    log::error!(
                        "Failed to create resources tracker ('{}') history revision, took {}: {:?}.",
                        tracker.id, humantime::format_duration(fetch_start.elapsed()), err
                    );
                    continue;
                }
            };
            log::debug!(
                "Successfully created resources tracker ('{}') history revision, took {}.",
                tracker.id,
                humantime::format_duration(fetch_start.elapsed())
            );

            if tracker.settings.enable_notifications {
                if let Some(new_revision_with_diff) = new_revision_with_diff {
                    let changes_count = new_revision_with_diff
                        .scripts
                        .iter()
                        .filter(|resource| resource.diff_status.is_some())
                        .chain(
                            new_revision_with_diff
                                .styles
                                .iter()
                                .filter(|resource| resource.diff_status.is_some()),
                        )
                        .count();
                    let notification_schedule_result = api
                        .notifications()
                        .schedule_notification(
                            NotificationDestination::User(tracker.user_id),
                            NotificationContent::Template(
                                NotificationContentTemplate::ResourcesTrackerChanges {
                                    tracker_name: tracker.name,
                                    changes_count,
                                },
                            ),
                            OffsetDateTime::now_utc(),
                        )
                        .await;
                    if let Err(err) = notification_schedule_result {
                        log::error!(
                            "Failed to schedule a notification for web page resources tracker ('{}'): {:?}.",
                            tracker.id, err
                        );
                    }
                }
            }

            api.db
                .set_scheduler_job_stopped_state(job_id, false)
                .await?;
        }

        let elapsed = execute_start.elapsed();
        if elapsed < JOB_EXECUTION_THRESHOLD {
            log::trace!(
                "Fetched resources for web page resources trackers ({} elapsed).",
                humantime::format_duration(elapsed),
            );
        } else {
            log::warn!(
                "Fetched resources for web page resources trackers ({} elapsed).",
                humantime::format_duration(elapsed),
            );
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
        tests::{mock_api_with_config, mock_config, mock_schedule_in_sec, mock_user},
        utils::{
            ResourcesCreateParams, WebPageResource, WebPageResourceContent,
            WebPageResourceContentData, WebPageResourcesRevision, WebPageResourcesTracker,
            WebPageResourcesTrackerScripts, WebPageResourcesTrackerSettings, WebScraperResource,
            WebScraperResourcesRequest, WebScraperResourcesRequestScripts,
            WebScraperResourcesResponse,
        },
    };
    use cron::Schedule;
    use futures::StreamExt;
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use std::{default::Default, ops::Add, sync::Arc, thread, time::Duration};
    use time::OffsetDateTime;
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
            extra: vec![SchedulerJob::ResourcesTrackersFetch as u8],
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[tokio::test]
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

    #[tokio::test]
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
    async fn remove_pending_trackers_jobs_if_zero_revisions() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch =
            Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

        let user = mock_user()?;
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
            .add(ResourcesTrackersTriggerJob::create(api.clone(), mock_schedule_in_sec(1)).await?)
            .await?;
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
            .update_resources_tracker_job(tracker.id, Some(tracker_job_id))
            .await?;

        // Schedule fetch job
        scheduler
            .add(ResourcesTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        let web_scraping = api.web_scraping();
        while web_scraping
            .get_resources_tracker_by_job_id(tracker_job_id)
            .await?
            .is_some()
        {
            thread::sleep(Duration::from_millis(100));
        }

        scheduler.shutdown().await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_fetch_resources() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch =
            Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

        let server = MockServer::start();
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let user = mock_user()?;
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

        // Make sure that the tracker is only run once during a single minute (2 seconds after the
        // current second).
        let tracker_schedule = mock_schedule_in_sec(1);
        let trigger_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), tracker_schedule.clone()).await?)
            .await?;
        let tracker = WebPageResourcesTracker {
            id: Uuid::now_v7(),
            name: "tracker".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageResourcesTrackerSettings {
                revisions: 1,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: WebPageResourcesTrackerScripts {
                    resource_filter_map: Some("return resource;".to_string()),
                },
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_resources_tracker(&tracker)
            .await?;

        // Schedule fetch job
        scheduler
            .add(ResourcesTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Create a mock
        let resources = WebScraperResourcesResponse {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebScraperResource {
                url: Some(Url::parse("http://localhost:1234/script.js")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 123,
                }),
            }],
            styles: vec![WebScraperResource {
                url: Some(Url::parse("http://localhost:1234/style.css")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-other-digest".to_string()),
                    size: 321,
                }),
            }],
        };

        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_scripts(WebScraperResourcesRequestScripts {
                                resource_filter_map: Some("return resource;"),
                            })
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources);
        });

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        let web_scraping = api.web_scraping();
        while web_scraping
            .get_resources_tracker_history(user.id, tracker.id, Default::default())
            .await?
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }

        scheduler.shutdown().await?;

        resources_mock.assert();

        // Check that resources were saved.
        assert_eq!(
            api.web_scraping()
                .get_resources_tracker_history(user.id, tracker.id, Default::default())
                .await?
                .into_iter()
                .map(|rev| (rev.created_at, rev.scripts, rev.styles))
                .collect::<Vec<_>>(),
            vec![(
                OffsetDateTime::from_unix_timestamp(946720800)?,
                resources
                    .scripts
                    .into_iter()
                    .map(WebPageResource::from)
                    .collect::<Vec<_>>(),
                resources
                    .styles
                    .into_iter()
                    .map(WebPageResource::from)
                    .collect::<Vec<_>>()
            )]
        );

        // Check that the tracker job was marked as NOT stopped.
        let trigger_job = api.db.get_scheduler_job(trigger_job_id).await?;
        assert_eq!(
            trigger_job.map(|job| (job.id, job.stopped)),
            Some((Some(trigger_job_id.into()), false))
        );

        assert!(api
            .db
            .get_notification_ids(
                OffsetDateTime::now_utc().add(Duration::from_secs(3600 * 24 * 365)),
                10
            )
            .collect::<Vec<_>>()
            .await
            .is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_notification_when_resources_change() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch =
            Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

        let server = MockServer::start();
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);
        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        // Make sure that the tracker is only run once during a single minute (2 seconds after the
        // current second).
        let tracker_schedule = mock_schedule_in_sec(1);

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;

        let trigger_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), tracker_schedule.clone()).await?)
            .await?;
        let tracker = WebPageResourcesTracker {
            id: Uuid::now_v7(),
            name: "tracker-one".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageResourcesTrackerSettings {
                revisions: 2,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Default::default(),
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_resources_tracker(&tracker)
            .await?;
        api.db
            .web_scraping(user.id)
            .insert_resources_tracker_history_revision(&WebPageResourcesRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: tracker.id,
                created_at: OffsetDateTime::from_unix_timestamp(946720700)?,
                scripts: vec![],
                styles: vec![],
            })
            .await?;

        // Schedule fetch job
        scheduler
            .add(ResourcesTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Create a mock
        let resources = WebScraperResourcesResponse {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebScraperResource {
                url: Some(Url::parse("http://localhost:1234/script.js")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 123,
                }),
            }],
            styles: vec![WebScraperResource {
                url: Some(Url::parse("http://localhost:1234/style.css")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-other-digest".to_string()),
                    size: 321,
                }),
            }],
        };

        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources);
        });

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        while api
            .db
            .get_notification_ids(
                OffsetDateTime::now_utc().add(Duration::from_secs(3600 * 24 * 365)),
                10,
            )
            .collect::<Vec<_>>()
            .await
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }

        scheduler.shutdown().await?;

        resources_mock.assert();

        let mut notification_ids = api
            .db
            .get_notification_ids(
                OffsetDateTime::now_utc().add(Duration::from_secs(3600 * 24 * 365)),
                10,
            )
            .collect::<Vec<_>>()
            .await;
        assert_eq!(notification_ids.len(), 1);

        let notification = api.db.get_notification(notification_ids.remove(0)?).await?;
        assert_debug_snapshot!(notification.map(|notification| (notification.destination, notification.content)), @r###"
        Some(
            (
                User(
                    UserId(
                        1,
                    ),
                ),
                Template(
                    ResourcesTrackerChanges {
                        tracker_name: "tracker-one",
                        changes_count: 2,
                    },
                ),
            ),
        )
        "###);

        assert_eq!(
            api.web_scraping()
                .get_resources_tracker_history(user.id, tracker.id, Default::default())
                .await?
                .len(),
            2
        );
        assert!(!api
            .db
            .get_scheduler_job(trigger_job_id)
            .await?
            .map(|job| job.stopped)
            .unwrap_or_default());

        Ok(())
    }
}
