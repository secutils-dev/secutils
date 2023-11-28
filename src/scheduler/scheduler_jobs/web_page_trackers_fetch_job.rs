use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    notifications::{NotificationContent, NotificationContentTemplate, NotificationDestination},
    scheduler::scheduler_job::SchedulerJob,
    utils::{WebPageTracker, WebPageTrackerTag},
};
use futures::{pin_mut, StreamExt};
use std::{sync::Arc, time::Instant};
use time::{Duration, OffsetDateTime};
use tokio_cron_scheduler::{Job, JobId, JobScheduler, JobStoredData};
use uuid::Uuid;

// If job execution takes more than 10 seconds, we'll log a warning instead of trace/debug message.
const JOB_EXECUTION_THRESHOLD: Duration = Duration::new(10, 0);

/// The job executes every minute by default to check if there are any trackers to fetch resources for.
pub(crate) struct WebPageTrackersFetchJob;
impl WebPageTrackersFetchJob {
    /// Tries to resume existing `WebPageTrackersFetch` job.
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

    /// Creates a new `WebPageTrackersFetch` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
    ) -> anyhow::Result<Job>
    where
        ET::Error: EmailTransportError,
    {
        let mut job = Job::new_async(
            api.config.jobs.web_page_trackers_fetch.clone(),
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
            extra: SchedulerJob::WebPageTrackersFetch.try_into()?,
            ..job_data
        })?;

        Ok(job)
    }

    /// Executes a `WebPageTrackersFetch` job.
    async fn execute<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()>
    where
        ET::Error: EmailTransportError,
    {
        let execute_start = Instant::now();

        Self::fetch_resources(api.clone(), scheduler.clone()).await?;
        Self::fetch_content(api, scheduler).await?;

        let elapsed = execute_start.elapsed();
        if elapsed < JOB_EXECUTION_THRESHOLD {
            log::trace!(
                "Fetched new web page tracker revisions ({} elapsed).",
                humantime::format_duration(elapsed),
            );
        } else {
            log::warn!(
                "Fetched new web page tracker revisions ({} elapsed).",
                humantime::format_duration(elapsed),
            );
        }

        Ok(())
    }

    async fn fetch_resources<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()>
    where
        ET::Error: EmailTransportError,
    {
        // Fetch all resources trackers jobs that are pending processing.
        let web_scraping = api.web_scraping();
        let pending_trackers = web_scraping.get_pending_resources_trackers();
        pin_mut!(pending_trackers);

        while let Some(tracker) = pending_trackers.next().await {
            let Some((tracker, job_id)) =
                Self::validate_tracker(&api, &scheduler, tracker?).await?
            else {
                continue;
            };

            // Check if resources has changed, comparing new revision to the latest existing one.
            let fetch_start = Instant::now();

            // Create a new revision and retrieve a diff if any changes from the previous version are
            // detected. If there are any changes and the tracker hasn't opted out of notifications,
            // schedule a notification about the detected changes.
            let new_revision_with_diff = match web_scraping
                .create_resources_tracker_revision(tracker.user_id, tracker.id)
                .await
            {
                Ok(new_revision_with_diff) => new_revision_with_diff,
                Err(err) => {
                    log::error!(
                        "Failed to create web page tracker ('{}') history revision, took {}: {:?}.",
                        tracker.id,
                        humantime::format_duration(fetch_start.elapsed()),
                        err
                    );

                    // Notify user about the error and re-schedule the job.
                    let tracker_name = tracker.name.clone();
                    Self::try_notify_user(
                        &api,
                        tracker,
                        NotificationContentTemplate::WebPageResourcesTrackerChanges {
                            tracker_name,
                            content: Err(err
                                .downcast::<SecutilsError>()
                                .map(|err| format!("{}", err))
                                .unwrap_or_else(|_| "Unknown error".to_string())),
                        },
                    )
                    .await;
                    api.db
                        .set_scheduler_job_stopped_state(job_id, false)
                        .await?;
                    continue;
                }
            };
            log::debug!(
                "Successfully created web page tracker ('{}') history revision, took {}.",
                tracker.id,
                humantime::format_duration(fetch_start.elapsed())
            );

            if tracker.settings.enable_notifications {
                if let Some(new_revision_with_diff) = new_revision_with_diff {
                    let changes_count = new_revision_with_diff
                        .data
                        .scripts
                        .iter()
                        .filter(|resource| resource.diff_status.is_some())
                        .chain(
                            new_revision_with_diff
                                .data
                                .styles
                                .iter()
                                .filter(|resource| resource.diff_status.is_some()),
                        )
                        .count();
                    let tracker_name = tracker.name.clone();
                    Self::try_notify_user(
                        &api,
                        tracker,
                        NotificationContentTemplate::WebPageResourcesTrackerChanges {
                            tracker_name,
                            content: Ok(changes_count),
                        },
                    )
                    .await;
                }
            }

            api.db
                .set_scheduler_job_stopped_state(job_id, false)
                .await?;
        }

        Ok(())
    }

    async fn fetch_content<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        scheduler: JobScheduler,
    ) -> anyhow::Result<()>
    where
        ET::Error: EmailTransportError,
    {
        // Fetch all content trackers jobs that are pending processing.
        let web_scraping = api.web_scraping();
        let pending_trackers = web_scraping.get_pending_content_trackers();
        pin_mut!(pending_trackers);

        while let Some(tracker) = pending_trackers.next().await {
            let Some((tracker, job_id)) =
                Self::validate_tracker(&api, &scheduler, tracker?).await?
            else {
                continue;
            };

            // Try to create a new revision. If a revision is returned that means that tracker
            // detected changes.
            let fetch_start = Instant::now();
            let new_revision = match web_scraping
                .create_content_tracker_revision(tracker.user_id, tracker.id)
                .await
            {
                Ok(new_revision) => new_revision,
                Err(err) => {
                    log::error!(
                        "Failed to create web page tracker ('{}') history revision, took {}: {:?}.",
                        tracker.id,
                        humantime::format_duration(fetch_start.elapsed()),
                        err
                    );

                    // Notify user about the error and re-schedule the job.
                    let tracker_name = tracker.name.clone();
                    Self::try_notify_user(
                        &api,
                        tracker,
                        NotificationContentTemplate::WebPageContentTrackerChanges {
                            tracker_name,
                            content: Err(err
                                .downcast::<SecutilsError>()
                                .map(|err| format!("{}", err))
                                .unwrap_or_else(|_| "Unknown error".to_string())),
                        },
                    )
                    .await;
                    api.db
                        .set_scheduler_job_stopped_state(job_id, false)
                        .await?;
                    continue;
                }
            };
            log::debug!(
                "Successfully created web page tracker ('{}') history revision, took {}.",
                tracker.id,
                humantime::format_duration(fetch_start.elapsed())
            );

            if let Some(revision) = new_revision {
                let tracker_name = tracker.name.clone();
                Self::try_notify_user(
                    &api,
                    tracker,
                    NotificationContentTemplate::WebPageContentTrackerChanges {
                        tracker_name,
                        content: Ok(revision.data),
                    },
                )
                .await;
            }

            api.db
                .set_scheduler_job_stopped_state(job_id, false)
                .await?;
        }

        Ok(())
    }

    async fn validate_tracker<DR: DnsResolver, ET: EmailTransport, Tag: WebPageTrackerTag>(
        api: &Api<DR, ET>,
        scheduler: &JobScheduler,
        tracker: WebPageTracker<Tag>,
    ) -> anyhow::Result<Option<(WebPageTracker<Tag>, Uuid)>>
    where
        ET::Error: EmailTransportError,
    {
        let Some(job_id) = tracker.job_id else {
            log::error!(
                "Could not find a job for a pending web page tracker ('{}'), skipping.",
                tracker.id
            );
            return Ok(None);
        };

        if tracker.settings.revisions == 0 || tracker.settings.schedule.is_none() {
            log::warn!(
                "Found a pending web page tracker ('{}') that doesn't support tracking, the job ('{:?}') will be removed.",
                tracker.id, tracker.job_id
            );
            scheduler.remove(&job_id).await?;
            api.web_scraping()
                .update_web_page_tracker_job(tracker.id, None)
                .await?;
            return Ok(None);
        }

        Ok(Some((tracker, job_id)))
    }

    async fn try_notify_user<DR: DnsResolver, ET: EmailTransport, Tag: WebPageTrackerTag>(
        api: &Api<DR, ET>,
        tracker: WebPageTracker<Tag>,
        template: NotificationContentTemplate,
    ) where
        ET::Error: EmailTransportError,
    {
        if !tracker.settings.enable_notifications {
            return;
        }

        let notification_schedule_result = api
            .notifications()
            .schedule_notification(
                NotificationDestination::User(tracker.user_id),
                NotificationContent::Template(template),
                OffsetDateTime::now_utc(),
            )
            .await;
        if let Err(err) = notification_schedule_result {
            log::error!(
                "Failed to schedule a notification for web page tracker ('{}'): {:?}.",
                tracker.id,
                err
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WebPageTrackersFetchJob;
    use crate::{
        scheduler::{
            scheduler_job::SchedulerJob, scheduler_jobs::WebPageTrackersTriggerJob,
            scheduler_store::SchedulerStore,
        },
        tests::{mock_api_with_config, mock_config, mock_schedule_in_sec, mock_user},
        utils::{
            WebPageContentTrackerTag, WebPageDataRevision, WebPageResource, WebPageResourceContent,
            WebPageResourceContentData, WebPageResourcesData, WebPageResourcesTrackerTag,
            WebPageTracker, WebPageTrackerCreateParams, WebPageTrackerKind, WebPageTrackerSettings,
            WebScraperContentRequest, WebScraperContentRequestScripts, WebScraperContentResponse,
            WebScraperErrorResponse, WebScraperResource, WebScraperResourcesRequest,
            WebScraperResourcesRequestScripts, WebScraperResourcesResponse,
            WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME,
            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
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
            extra: SchedulerJob::WebPageTrackersFetch.try_into().unwrap(),
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[tokio::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_fetch = Schedule::try_from("1/5 * * * * *")?;

        let api = mock_api_with_config(config).await?;

        let mut job = WebPageTrackersFetchJob::create(Arc::new(api)).await?;
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
        config.jobs.web_page_trackers_fetch = Schedule::try_from("0 0 * * * *")?;

        let api = mock_api_with_config(config).await?;

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        let job = WebPageTrackersFetchJob::try_resume(Arc::new(api), job_id, mock_job_data(job_id))
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
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(2).as_str())?;

        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(config).await?);
        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        // Create user and trackers.
        api.users().upsert(user.clone()).await?;
        let resources_tracker_job_id = scheduler
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    mock_schedule_in_sec(1),
                    WebPageTrackerKind::WebPageResources,
                )
                .await?,
            )
            .await?;
        let resources_tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 0,
                        schedule: Some("0 0 * * * *".to_string()),
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                        enable_notifications: true,
                    },
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(resources_tracker.id, Some(resources_tracker_job_id))
            .await?;

        let content_tracker_job_id = scheduler
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    mock_schedule_in_sec(1),
                    WebPageTrackerKind::WebPageContent,
                )
                .await?,
            )
            .await?;
        let content_tracker = api
            .web_scraping()
            .create_content_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 0,
                        schedule: Some("0 0 * * * *".to_string()),
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                        enable_notifications: true,
                    },
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(content_tracker.id, Some(content_tracker_job_id))
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        let web_scraping = api.web_scraping();
        while web_scraping
            .get_resources_tracker_by_job_id(resources_tracker_job_id)
            .await?
            .is_some()
            || web_scraping
                .get_content_tracker_by_job_id(content_tracker_job_id)
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
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

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
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    tracker_schedule.clone(),
                    WebPageTrackerKind::WebPageResources,
                )
                .await?,
            )
            .await?;
        let tracker = WebPageTracker::<WebPageResourcesTrackerTag> {
            id: Uuid::now_v7(),
            name: "tracker".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 1,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Some(
                    [(
                        WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                        "return resource;".to_string(),
                    )]
                    .into_iter()
                    .collect(),
                ),
                headers: Some(
                    [("cookie".to_string(), "my-cookie".to_string())]
                        .into_iter()
                        .collect(),
                ),
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker(&tracker)
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
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
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_scripts(WebScraperResourcesRequestScripts {
                                resource_filter_map: Some("return resource;"),
                            })
                            .set_headers(
                                &[("cookie".to_string(), "my-cookie".to_string())]
                                    .into_iter()
                                    .collect(),
                            )
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
                .map(|rev| (rev.created_at, rev.data.scripts, rev.data.styles))
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
    async fn can_fetch_content() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

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
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    tracker_schedule.clone(),
                    WebPageTrackerKind::WebPageContent,
                )
                .await?,
            )
            .await?;
        let tracker = WebPageTracker::<WebPageContentTrackerTag> {
            id: Uuid::now_v7(),
            name: "tracker".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 1,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Some(
                    [(
                        WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME.to_string(),
                        "return document.body.innerText;".to_string(),
                    )]
                    .into_iter()
                    .collect(),
                ),
                headers: Some(
                    [("cookie".to_string(), "my-cookie".to_string())]
                        .into_iter()
                        .collect(),
                ),
                enable_notifications: false,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker(&tracker)
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Create a mock
        let content = WebScraperContentResponse {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            content: "some-content".to_string(),
        };

        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_scripts(WebScraperContentRequestScripts {
                                extract_content: Some("return document.body.innerText;"),
                            })
                            .set_headers(
                                &[("cookie".to_string(), "my-cookie".to_string())]
                                    .into_iter()
                                    .collect(),
                            )
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content);
        });

        // Start scheduler and wait for a few seconds, then stop it.
        scheduler.start().await?;

        let web_scraping = api.web_scraping();
        while web_scraping
            .get_content_tracker_history(user.id, tracker.id, Default::default())
            .await?
            .is_empty()
        {
            thread::sleep(Duration::from_millis(100));
        }

        scheduler.shutdown().await?;

        content_mock.assert();

        // Check that content was saved.
        assert_eq!(
            api.web_scraping()
                .get_content_tracker_history(user.id, tracker.id, Default::default())
                .await?
                .into_iter()
                .map(|rev| (rev.created_at, rev.data))
                .collect::<Vec<_>>(),
            vec![(
                OffsetDateTime::from_unix_timestamp(946720800)?,
                content.content
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
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

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
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    tracker_schedule.clone(),
                    WebPageTrackerKind::WebPageResources,
                )
                .await?,
            )
            .await?;
        let tracker = WebPageTracker::<WebPageResourcesTrackerTag> {
            id: Uuid::now_v7(),
            name: "tracker-one".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 2,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Default::default(),
                headers: Default::default(),
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker(&tracker)
            .await?;
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_id: tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720700)?,
                    data: WebPageResourcesData {
                        scripts: vec![],
                        styles: vec![],
                    },
                },
            )
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
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
                .path("/api/web_page/resources")
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
                    WebPageResourcesTrackerChanges {
                        tracker_name: "tracker-one",
                        content: Ok(
                            2,
                        ),
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_notification_when_resources_change_check_fails() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

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
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    tracker_schedule.clone(),
                    WebPageTrackerKind::WebPageResources,
                )
                .await?,
            )
            .await?;
        let tracker = WebPageTracker::<WebPageResourcesTrackerTag> {
            id: Uuid::now_v7(),
            name: "tracker-one".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 2,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Default::default(),
                headers: Default::default(),
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker(&tracker)
            .await?;
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_id: tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720700)?,
                    data: WebPageResourcesData {
                        scripts: vec![],
                        styles: vec![],
                    },
                },
            )
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
            .await?;

        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(400)
                .header("Content-Type", "application/json")
                .json_body_obj(&WebScraperErrorResponse {
                    message: "some client-error".to_string(),
                });
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
                    WebPageResourcesTrackerChanges {
                        tracker_name: "tracker-one",
                        content: Err(
                            "some client-error",
                        ),
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
            1
        );
        assert!(!api
            .db
            .get_scheduler_job(trigger_job_id)
            .await?
            .map(|job| job.stopped)
            .unwrap_or_default());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_notification_when_content_change() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

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
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    tracker_schedule.clone(),
                    WebPageTrackerKind::WebPageContent,
                )
                .await?,
            )
            .await?;
        let tracker = WebPageTracker::<WebPageContentTrackerTag> {
            id: Uuid::now_v7(),
            name: "tracker-one".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 2,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Default::default(),
                headers: Default::default(),
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker(&tracker)
            .await?;
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_id: tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720700)?,
                    data: "some-content".to_string(),
                },
            )
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
            .await?;

        // Create a mock
        let resources = WebScraperContentResponse {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            content: "other-content".to_string(),
        };

        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000))
                            .set_previous_content("some-content"),
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

        content_mock.assert();

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
                    WebPageContentTrackerChanges {
                        tracker_name: "tracker-one",
                        content: Ok(
                            "other-content",
                        ),
                    },
                ),
            ),
        )
        "###);

        assert_eq!(
            api.web_scraping()
                .get_content_tracker_history(user.id, tracker.id, Default::default())
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedules_notification_when_content_change_check_fails() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.jobs.web_page_trackers_fetch = Schedule::try_from(mock_schedule_in_sec(3).as_str())?;

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
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    tracker_schedule.clone(),
                    WebPageTrackerKind::WebPageContent,
                )
                .await?,
            )
            .await?;
        let tracker = WebPageTracker::<WebPageContentTrackerTag> {
            id: Uuid::now_v7(),
            name: "tracker-one".to_string(),
            url: "https://localhost:1234/my/app?q=2".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 2,
                schedule: Some(tracker_schedule),
                delay: Duration::from_secs(2),
                scripts: Default::default(),
                headers: Default::default(),
                enable_notifications: true,
            },
            user_id: user.id,
            job_id: Some(trigger_job_id),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        // Insert tracker directly to DB to bypass schedule validation.
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker(&tracker)
            .await?;
        api.db
            .web_scraping(user.id)
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_id: tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720700)?,
                    data: "some-content".to_string(),
                },
            )
            .await?;

        // Schedule fetch job
        scheduler
            .add(WebPageTrackersFetchJob::create(api.clone()).await?)
            .await?;

        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000))
                            .set_previous_content("some-content"),
                    )
                    .unwrap(),
                );
            then.status(400)
                .header("Content-Type", "application/json")
                .json_body_obj(&WebScraperErrorResponse {
                    message: "some client-error".to_string(),
                });
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

        content_mock.assert();

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
                    WebPageContentTrackerChanges {
                        tracker_name: "tracker-one",
                        content: Err(
                            "some client-error",
                        ),
                    },
                ),
            ),
        )
        "###);

        assert_eq!(
            api.web_scraping()
                .get_content_tracker_history(user.id, tracker.id, Default::default())
                .await?
                .len(),
            1
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