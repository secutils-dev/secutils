use crate::{
    api::Api,
    network::DnsResolver,
    notifications::{NotificationContent, NotificationDestination},
    scheduler::scheduler_job::SchedulerJob,
    users::UserData,
    utils::web_page_resources_revisions_diff,
};
use futures::{pin_mut, StreamExt};
use std::{sync::Arc, time::Instant};
use time::OffsetDateTime;
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
        let execute_start = Instant::now();

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

            // Check if resources has changed, comparing new revision to the latest existing one.
            let fetch_start = Instant::now();
            let new_revision = match web_scraping.fetch_resources(&tracker).await {
                Ok(new_revision) => new_revision,
                Err(err) => {
                    log::error!(
                        "Failed to fetch resources for web page resources tracker {} ({}, user: {:?}, {} elapsed): {:?}.",
                        tracker.name, tracker.url, user_id, err, humantime::format_duration(fetch_start.elapsed()),
                    );
                    continue;
                }
            };
            log::debug!(
                "Fetched resources for web page resources tracker {} ({}, user: {:?}, {} elapsed).",
                tracker.name,
                tracker.url,
                user_id,
                humantime::format_duration(fetch_start.elapsed())
            );

            let mut existing_revisions = web_scraping.get_resources(user_id, &tracker).await?;
            let changes_detected = if let Some(latest_revision) = existing_revisions.pop() {
                matches!(
                    web_page_resources_revisions_diff(vec![latest_revision, new_revision.clone()])?.get(1),
                    Some(diff) if diff.has_diff()
                )
            } else {
                false
            };

            // Save new revision.
            web_scraping
                .save_resources(user_id, &tracker, new_revision)
                .await?;

            // If changes are detected, schedule a notification.
            if changes_detected {
                let destination = NotificationDestination::User(user_id);
                let content = NotificationContent::String(format!(
                    "Web page resources tracker {} ({}, user: {:?}) detected changes in resources.",
                    tracker.name, tracker.url, user_id
                ));

                if let Err(err) = api
                    .notifications()
                    .schedule_notification(destination, content, OffsetDateTime::now_utc())
                    .await
                {
                    log::error!(
                        "Failed to schedule a notification for web page resources tracker {} ({}, user: {:?}): {:?}.",
                        tracker.name, tracker.url, user_id, err
                    );
                }
            }

            api.db
                .set_scheduler_job_stopped_state(tracker_job_id, false)
                .await?;
        }

        log::debug!(
            "Fetched resources for web page resources trackers ({} elapsed).",
            humantime::format_duration(execute_start.elapsed()),
        );

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
            WebPageResourceContent, WebPageResourceContentData, WebPageResourcesRevision,
            WebPageResourcesTracker, WebScraperResource, WebScraperResourcesRequest,
            WebScraperResourcesResponse,
        },
    };
    use cron::Schedule;
    use futures::StreamExt;
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use std::{ops::Add, sync::Arc, time::Duration};
    use time::OffsetDateTime;
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
        config.jobs.resources_trackers_fetch =
            Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

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
            .add(ResourcesTrackersTriggerJob::create(api.clone(), mock_schedule_in_sec(1)).await?)
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

        // There shouldn't be a tracker job anymore.
        let web_scraping = api.web_scraping();
        while web_scraping
            .get_resources_tracker_job_by_id(tracker_job_id)
            .await?
            .is_some()
        {}

        scheduler.shutdown().await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_pending_trackers_jobs_if_schedule_removed() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch =
            Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

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
            .add(ResourcesTrackersTriggerJob::create(api.clone(), mock_schedule_in_sec(1)).await?)
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

        // There shouldn't be a tracker job anymore.
        let web_scraping = api.web_scraping();
        while web_scraping
            .get_resources_tracker_job_by_id(tracker_job_id)
            .await?
            .is_some()
        {}

        scheduler.shutdown().await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remove_pending_trackers_jobs_if_zero_revisions() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.resources_trackers_fetch =
            Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

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
            .add(ResourcesTrackersTriggerJob::create(api.clone(), mock_schedule_in_sec(1)).await?)
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

        let web_scraping = api.web_scraping();
        while web_scraping
            .get_resources_tracker_job_by_id(tracker_job_id)
            .await?
            .is_some()
        {}

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

        let user = mock_user();
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

        let tracker = api
            .web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 1,
                    delay: Duration::from_millis(2000),
                    schedule: Some(tracker_schedule.clone()),
                },
            )
            .await?;
        let trigger_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), tracker_schedule).await?)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", Some(trigger_job_id))
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
                    serde_json::to_value(WebScraperResourcesRequest {
                        url: &tracker.url,
                        timeout: None,
                        delay: Some(2000),
                        wait_selector: None,
                    })
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
            .get_resources(user.id, &tracker)
            .await?
            .is_empty()
        {}

        scheduler.shutdown().await?;

        resources_mock.assert();

        // Check that resources were saved.
        assert_debug_snapshot!(api.web_scraping().get_resources(user.id, &tracker).await?,  @r###"
        [
            WebPageResourcesRevision {
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
                scripts: [
                    WebPageResource {
                        url: Some(
                            Url {
                                scheme: "http",
                                cannot_be_a_base: false,
                                username: "",
                                password: None,
                                host: Some(
                                    Domain(
                                        "localhost",
                                    ),
                                ),
                                port: Some(
                                    1234,
                                ),
                                path: "/script.js",
                                query: None,
                                fragment: None,
                            },
                        ),
                        content: Some(
                            WebPageResourceContent {
                                data: Sha1(
                                    "some-digest",
                                ),
                                size: 123,
                            },
                        ),
                        diff_status: None,
                    },
                ],
                styles: [
                    WebPageResource {
                        url: Some(
                            Url {
                                scheme: "http",
                                cannot_be_a_base: false,
                                username: "",
                                password: None,
                                host: Some(
                                    Domain(
                                        "localhost",
                                    ),
                                ),
                                port: Some(
                                    1234,
                                ),
                                path: "/style.css",
                                query: None,
                                fragment: None,
                            },
                        ),
                        content: Some(
                            WebPageResourceContent {
                                data: Sha1(
                                    "some-other-digest",
                                ),
                                size: 321,
                            },
                        ),
                        diff_status: None,
                    },
                ],
            },
        ]
        "###);

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

        let user = mock_user();
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

        let tracker = api
            .web_scraping()
            .upsert_resources_tracker(
                user.id,
                WebPageResourcesTracker {
                    name: "tracker-one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 2,
                    delay: Duration::from_millis(2000),
                    schedule: Some(tracker_schedule.clone()),
                },
            )
            .await?;
        api.web_scraping()
            .save_resources(
                user.id,
                &tracker,
                WebPageResourcesRevision {
                    timestamp: OffsetDateTime::from_unix_timestamp(946720700)?,
                    scripts: vec![],
                    styles: vec![],
                },
            )
            .await?;
        let trigger_job_id = scheduler
            .add(ResourcesTrackersTriggerJob::create(api.clone(), tracker_schedule).await?)
            .await?;
        api.web_scraping()
            .upsert_resources_tracker_job(user.id, "tracker-one", Some(trigger_job_id))
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
                    serde_json::to_value(WebScraperResourcesRequest {
                        url: &tracker.url,
                        timeout: None,
                        delay: Some(2000),
                        wait_selector: None,
                    })
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources);
        });

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        let web_scraping = api.web_scraping();
        while web_scraping.get_resources(user.id, &tracker).await?.len() < 2
            || !api
                .db
                .get_scheduler_job(trigger_job_id)
                .await?
                .map(|job| job.stopped)
                .unwrap_or_default()
        {}

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
                String(
                    "Web page resources tracker tracker-one (http://localhost:1234/my/app?q=2, user: UserId(1)) detected changes in resources.",
                ),
            ),
        )
        "###);

        Ok(())
    }
}