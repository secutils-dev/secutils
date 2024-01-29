use crate::{
    api::Api,
    logging::JobLogContext,
    network::{DnsResolver, EmailTransport},
    scheduler::{job_ext::JobExt, scheduler_job::SchedulerJob},
    utils::web_scraping::WebPageTrackerKind,
};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobId, JobStoredData};

/// The job that is executed for every web page tracker with automatic tracking enabled. The
/// job doesn't do anything except logging, and updating its internal state. This job is supposed to
/// be as lightweight as possible since we might have thousands of them. There are dedicated
/// schedule and fetch jobs that batch all trackers that need to be scheduled and checked for
/// changes respectively.
pub(crate) struct WebPageTrackersTriggerJob;
impl WebPageTrackersTriggerJob {
    /// Tries to resume existing `ResourcesTrackersTrigger` job.
    pub async fn try_resume<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        job_id: JobId,
        existing_job_data: JobStoredData,
        tracker_kind: WebPageTrackerKind,
    ) -> anyhow::Result<Option<Job>> {
        // First, check if the tracker job exists.
        let web_scraping = api.web_scraping();
        let Some((tracker_id, tracker_settings, tracker_job_config)) = (match tracker_kind {
            WebPageTrackerKind::WebPageResources => web_scraping
                .get_resources_tracker_by_job_id(job_id)
                .await?
                .map(|tracker| (tracker.id, tracker.settings, tracker.job_config)),
            WebPageTrackerKind::WebPageContent => web_scraping
                .get_content_tracker_by_job_id(job_id)
                .await?
                .map(|tracker| (tracker.id, tracker.settings, tracker.job_config)),
        }) else {
            log::warn!(
                job = log::as_serde!(JobLogContext::new(job_id));
                "Web page tracker job reference doesn't exist, the job will be removed."
            );
            return Ok(None);
        };

        // Then, check if the tracker can support revisions.
        if tracker_settings.revisions == 0 {
            log::warn!(
                "Web page tracker ('{}') no cannot store revisions, the job ('{job_id}') will be removed.",
                tracker_id
            );
            web_scraping
                .update_web_page_tracker_job(tracker_id, None)
                .await?;
            return Ok(None);
        };

        // Then, check if the tracker still has a schedule.
        let Some(job_config) = tracker_job_config else {
            log::warn!(
                "Web page tracker ('{}') no longer has a job config, the job ('{job_id}') will be removed.",
                tracker_id
            );
            web_scraping
                .update_web_page_tracker_job(tracker_id, None)
                .await?;
            return Ok(None);
        };

        // If we changed the job parameters, we need to remove the old job and create a new one.
        let mut new_job = Self::create(api.clone(), job_config.schedule, tracker_kind).await?;
        Ok(if new_job.job_data()?.job == existing_job_data.job {
            new_job.set_job_data(existing_job_data)?;
            Some(new_job)
        } else {
            web_scraping
                .update_web_page_tracker_job(tracker_id, None)
                .await?;
            None
        })
    }

    /// Creates a new `WebPageTrackersTrigger` job.
    pub async fn create<DR: DnsResolver, ET: EmailTransport>(
        api: Arc<Api<DR, ET>>,
        schedule: impl AsRef<str>,
        tracker_kind: WebPageTrackerKind,
    ) -> anyhow::Result<Job> {
        // Now, create and schedule new job.
        let mut job = Job::new_async(schedule.as_ref(), move |uuid, _| {
            let db = api.db.clone();
            Box::pin(async move {
                // Mark job as stopped to indicate that it needs processing. Schedule job only picks
                // up stopped jobs, processes them, and then un-stops. Stopped flag is basically
                // serving as a pending processing flag. Eventually we might need to add a separate
                // table for pending jobs.
                if let Err(err) = db.reset_scheduler_job_state(uuid, true).await {
                    log::error!(
                        job = log::as_serde!(JobLogContext::new(uuid));
                        "Error marking web page tracker trigger job as pending: {err:?}"
                    );
                } else {
                    log::debug!(
                        job = log::as_serde!(JobLogContext::new(uuid));
                        "Successfully run the job."
                    );
                }
            })
        })?;

        job.set_job_type(SchedulerJob::WebPageTrackersTrigger { kind: tracker_kind })?;

        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::WebPageTrackersTriggerJob;
    use crate::{
        scheduler::{
            scheduler_job::SchedulerJob, scheduler_store::SchedulerStore, SchedulerJobConfig,
            SchedulerJobMetadata,
        },
        tests::{mock_api, mock_user},
        utils::web_scraping::{
            tests::WebPageTrackerCreateParams, WebPageTracker, WebPageTrackerKind,
            WebPageTrackerSettings,
        },
    };
    use insta::assert_debug_snapshot;
    use std::sync::Arc;
    use tokio_cron_scheduler::{
        CronJob, JobId, JobScheduler, JobStored, JobStoredData, JobType, SimpleJobCode,
        SimpleNotificationCode, SimpleNotificationStore,
    };
    use uuid::uuid;

    fn mock_job_data(job_id: JobId, job_type: SchedulerJob) -> JobStoredData {
        JobStoredData {
            id: Some(job_id.into()),
            job_type: JobType::Cron as i32,
            count: 0,
            last_tick: None,
            next_tick: 100500,
            ran: false,
            stopped: false,
            last_updated: None,
            extra: SchedulerJobMetadata::new(job_type).try_into().unwrap(),
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 * * * *".to_string(),
            })),
        }
    }

    #[tokio::test]
    async fn can_create_job_with_correct_parameters() -> anyhow::Result<()> {
        let api = Arc::new(mock_api().await?);

        let mut job_data = vec![];
        for tracker_kind in [
            WebPageTrackerKind::WebPageResources,
            WebPageTrackerKind::WebPageContent,
        ] {
            let mut job =
                WebPageTrackersTriggerJob::create(api.clone(), "0 0 * * * *", tracker_kind).await?;
            job_data.push(job.job_data().map(|job_data| {
                (
                    job_data.job_type,
                    job_data.extra,
                    job_data.job,
                    job_data.stopped,
                )
            })?);
        }

        assert_debug_snapshot!(job_data, @r###"
        [
            (
                0,
                [
                    0,
                    0,
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
            ),
            (
                0,
                [
                    0,
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
                false,
            ),
        ]
        "###);

        Ok(())
    }

    #[tokio::test]
    async fn can_resume_resources_job() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker = api
            .web_scraping()
            .create_resources_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 4,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(tracker.id, Some(job_id))
            .await?;

        let mut job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
            ),
            WebPageTrackerKind::WebPageResources,
        )
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
                0,
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
    async fn can_resume_content_job() -> anyhow::Result<()> {
        let user = mock_user()?;
        let api = Arc::new(mock_api().await?);

        let job_id = uuid!("00000000-0000-0000-0000-000000000000");

        // Create user, tracker and tracker job.
        api.users().upsert(user.clone()).await?;
        let tracker = api
            .web_scraping()
            .create_content_tracker(
                user.id,
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 4,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(tracker.id, Some(job_id))
            .await?;

        let mut job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                },
            ),
            WebPageTrackerKind::WebPageContent,
        )
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
        )
        "###);

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_content_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        assert_eq!(
            api.web_scraping()
                .get_content_tracker_by_job_id(job_id)
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
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 4,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "1 0 * * * *".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(tracker.id, Some(job_id))
            .await?;

        let job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
            ),
            WebPageTrackerKind::WebPageResources,
        )
        .await?;
        assert!(job.is_none());

        let unscheduled_trackers = api
            .web_scraping()
            .get_unscheduled_resources_trackers()
            .await?;
        assert_eq!(
            unscheduled_trackers,
            vec![WebPageTracker {
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
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 4,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: None,
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(tracker.id, Some(job_id))
            .await?;

        let job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
            ),
            WebPageTrackerKind::WebPageResources,
        )
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
                WebPageTrackerCreateParams {
                    name: "tracker".to_string(),
                    url: "https://localhost:1234/my/app?q=2".parse()?,
                    settings: WebPageTrackerSettings {
                        revisions: 0,
                        delay: Default::default(),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    job_config: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    }),
                },
            )
            .await?;
        api.web_scraping()
            .update_web_page_tracker_job(tracker.id, Some(job_id))
            .await?;

        let job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
            ),
            WebPageTrackerKind::WebPageResources,
        )
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

        let job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
            ),
            WebPageTrackerKind::WebPageResources,
        )
        .await?;
        assert!(job.is_none());

        let job = WebPageTrackersTriggerJob::try_resume(
            api.clone(),
            job_id,
            mock_job_data(
                job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                },
            ),
            WebPageTrackerKind::WebPageContent,
        )
        .await?;
        assert!(job.is_none());

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

        assert!(api
            .web_scraping()
            .get_resources_tracker_by_job_id(job_id)
            .await?
            .is_none());
        assert!(api
            .web_scraping()
            .get_content_tracker_by_job_id(job_id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn marks_resources_job_as_stopped_when_run() -> anyhow::Result<()> {
        let api = Arc::new(mock_api().await?);

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        let trigger_job_id = scheduler
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    "1/1 * * * * *",
                    WebPageTrackerKind::WebPageResources,
                )
                .await?,
            )
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn marks_content_job_as_stopped_when_run() -> anyhow::Result<()> {
        let api = Arc::new(mock_api().await?);

        let mut scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(api.db.clone())),
            Box::<SimpleNotificationStore>::default(),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        let trigger_job_id = scheduler
            .add(
                WebPageTrackersTriggerJob::create(
                    api.clone(),
                    "1/1 * * * * *",
                    WebPageTrackerKind::WebPageContent,
                )
                .await?,
            )
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
