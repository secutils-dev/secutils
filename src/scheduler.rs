mod api_ext;
mod cron_ext;
mod database_ext;
mod job_ext;
mod scheduler_job;
mod scheduler_job_config;
mod scheduler_job_metadata;
mod scheduler_job_retry_state;
mod scheduler_job_retry_strategy;
mod scheduler_jobs;

pub use self::{
    cron_ext::CronExt, scheduler_job::SchedulerJob, scheduler_job_config::SchedulerJobConfig,
    scheduler_job_metadata::SchedulerJobMetadata,
    scheduler_job_retry_state::SchedulerJobRetryState,
    scheduler_job_retry_strategy::SchedulerJobRetryStrategy,
};
use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    scheduler::scheduler_jobs::{
        NotificationsSendJob, WebPageTrackersFetchJob, WebPageTrackersScheduleJob,
        WebPageTrackersTriggerJob,
    },
};
use anyhow::anyhow;
use futures::{StreamExt, pin_mut};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{
    JobScheduler, PostgresMetadataStore, PostgresNotificationStore, PostgresStore, SimpleJobCode,
    SimpleNotificationCode,
};
use tracing::{debug, error, warn};

/// Defines a maximum number of jobs that can be retrieved from the database at once.
const MAX_JOBS_PAGE_SIZE: usize = 1000;

/// Defines a name of the table that stores scheduler jobs.
const SCHEDULER_JOBS_TABLE: &str = "scheduler_jobs";

/// Defines a name of the table that stores scheduler notifications.
const SCHEDULER_NOTIFICATIONS_TABLE: &str = "scheduler_notifications";

/// Defines a name of the table that stores scheduler notification states.
const SCHEDULER_NOTIFICATION_STATES_TABLE: &str = "scheduler_notification_states";

/// The scheduler is responsible for scheduling and executing Secutils.dev jobs.
pub struct Scheduler<DR: DnsResolver, ET: EmailTransport> {
    inner_scheduler: JobScheduler,
    api: Arc<Api<DR, ET>>,
}

impl<DR: DnsResolver, ET: EmailTransport> Scheduler<DR, ET>
where
    ET::Error: EmailTransportError,
{
    /// Starts the scheduler resuming existing jobs and adding new ones.
    pub async fn start(api: Arc<Api<DR, ET>>) -> anyhow::Result<Self> {
        let store_db_config = &api.config.db;
        let store = Arc::new(RwLock::new(PostgresStore::Created(format!(
            "host={} port={} dbname={} {}",
            store_db_config.host,
            store_db_config.port,
            store_db_config.name,
            if let Some(password) = &store_db_config.password {
                format!("user={} password={password}", store_db_config.username)
            } else {
                format!("user={}", store_db_config.username)
            }
        ))));
        let metadata_store = PostgresMetadataStore {
            store: store.clone(),
            init_tables: false,
            table: SCHEDULER_JOBS_TABLE.to_string(),
        };

        let scheduler = Self {
            inner_scheduler: JobScheduler::new_with_storage_and_code(
                Box::new(metadata_store),
                Box::new(PostgresNotificationStore {
                    store,
                    init_tables: false,
                    table: SCHEDULER_NOTIFICATIONS_TABLE.to_string(),
                    states_table: SCHEDULER_NOTIFICATION_STATES_TABLE.to_string(),
                }),
                Box::<SimpleJobCode>::default(),
                Box::<SimpleNotificationCode>::default(),
                1000,
            )
            .await?,
            api,
        };

        // First, try to resume existing jobs.
        let resumed_unique_jobs = scheduler.resume().await?;
        if !resumed_unique_jobs.contains(&SchedulerJob::WebPageTrackersSchedule) {
            scheduler
                .inner_scheduler
                .add(WebPageTrackersScheduleJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        if !resumed_unique_jobs.contains(&SchedulerJob::WebPageTrackersFetch) {
            scheduler
                .inner_scheduler
                .add(WebPageTrackersFetchJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        if !resumed_unique_jobs.contains(&SchedulerJob::NotificationsSend) {
            scheduler
                .inner_scheduler
                .add(NotificationsSendJob::create(scheduler.api.clone()).await?)
                .await?;
        }

        scheduler.inner_scheduler.start().await?;
        Ok(scheduler)
    }

    /// Resumes existing jobs.
    async fn resume(&self) -> anyhow::Result<HashSet<SchedulerJob>> {
        let db = &self.api.db;
        let jobs = db.get_scheduler_jobs(MAX_JOBS_PAGE_SIZE);
        pin_mut!(jobs);

        // Track jobs for the job types that should be scheduled only once.
        let mut unique_resumed_jobs = HashSet::new();
        while let Some(job_data) = jobs.next().await {
            let job_data = job_data?;
            let job_id = job_data.id;
            let job_meta = job_data
                .extra
                .as_ref()
                .ok_or_else(|| anyhow!("Job `{job_data:?}` doesnt have extra data"))
                .and_then(|extra| SchedulerJobMetadata::try_from(extra.as_ref()));
            let job_meta = match job_meta {
                Ok(job_meta) if unique_resumed_jobs.contains(&job_meta.job_type) => {
                    // There can only be one job of each type. If we detect that there are multiple, we log
                    // a warning and remove the job, keeping only the first one.
                    error!(
                        "Found multiple jobs of type `{:?}`. All duplicated jobs except for the first one will be removed.",
                        job_meta.job_type
                    );
                    self.inner_scheduler.remove(&job_id).await?;
                    continue;
                }
                Err(err) => {
                    // We don't fail here, because we want to gracefully handle the legacy jobs.
                    error!(
                        "Failed to deserialize job type for job `{job_data:?}`: {err:?}. The job will be removed."
                    );
                    self.inner_scheduler.remove(&job_id).await?;
                    continue;
                }
                Ok(job_meta) => job_meta,
            };

            // First try to resume the job, and if it's not possible, the job will be removed and
            // re-scheduled at a later step if needed.
            let job = match &job_meta.job_type {
                SchedulerJob::WebPageTrackersTrigger { kind } => {
                    WebPageTrackersTriggerJob::try_resume(self.api.clone(), job_data, *kind).await?
                }
                SchedulerJob::WebPageTrackersSchedule => {
                    WebPageTrackersScheduleJob::try_resume(self.api.clone(), job_data).await?
                }
                SchedulerJob::WebPageTrackersFetch => {
                    WebPageTrackersFetchJob::try_resume(self.api.clone(), job_data).await?
                }
                SchedulerJob::NotificationsSend => {
                    NotificationsSendJob::try_resume(self.api.clone(), job_data).await?
                }
            };

            match job {
                Some(job) => {
                    debug!("Resumed job (`{:?}`): {}.", job_meta.job_type, job_id);
                    self.inner_scheduler.add(job).await?;

                    if job_meta.job_type.is_unique() {
                        unique_resumed_jobs.insert(job_meta.job_type);
                    }
                }
                None => {
                    warn!(
                        "Failed to resume job (`{:?}`): {job_id}. The job will be removed and re-scheduled if needed.",
                        job_meta.job_type
                    );
                    self.inner_scheduler.remove(&job_id).await?;
                }
            }
        }

        Ok(unique_resumed_jobs)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        config::{Config, DatabaseConfig},
        scheduler::{
            Scheduler, SchedulerJobConfig, SchedulerJobMetadata, scheduler_job::SchedulerJob,
        },
        tests::{mock_api_with_config, mock_config, mock_user},
        utils::web_scraping::{
            WebPageTrackerKind, WebPageTrackerSettings, tests::WebPageTrackerCreateParams,
        },
    };
    use anyhow::anyhow;
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::{env, sync::Arc, vec::Vec};
    use tokio::sync::RwLock;
    use tokio_cron_scheduler::{
        JobScheduler, PostgresMetadataStore, PostgresNotificationStore, PostgresStore,
        SimpleJobCode, SimpleNotificationCode,
    };
    use uuid::{Uuid, uuid};

    pub use super::database_ext::tests::{
        RawSchedulerJobStoredData, mock_get_scheduler_job, mock_upsert_scheduler_job,
    };

    pub async fn mock_scheduler(pool: &PgPool) -> anyhow::Result<JobScheduler> {
        let connect_options = pool.connect_options();
        let store = Arc::new(RwLock::new(PostgresStore::Created(format!(
            "host={} port={} dbname={} {}",
            connect_options.get_host(),
            connect_options.get_port(),
            connect_options
                .get_database()
                .ok_or_else(|| anyhow!("Database name is not available"))?,
            if let Ok(password) = env::var("DATABASE_PASSWORD") {
                format!(
                    "user={} password={password}",
                    connect_options.get_username()
                )
            } else {
                format!("user={}", connect_options.get_username())
            }
        ))));

        let metadata_store = PostgresMetadataStore {
            store: store.clone(),
            init_tables: false,
            table: "scheduler_jobs".to_string(),
        };

        Ok(JobScheduler::new_with_storage_and_code(
            Box::new(metadata_store),
            Box::new(PostgresNotificationStore {
                store,
                init_tables: false,
                table: "scheduler_notifications".to_string(),
                states_table: "scheduler_notification_states".to_string(),
            }),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
            1000,
        )
        .await?)
    }

    async fn mock_scheduler_config(pool: &PgPool) -> anyhow::Result<Config> {
        let connect_options = pool.connect_options();
        Ok(Config {
            db: DatabaseConfig {
                name: connect_options
                    .get_database()
                    .ok_or_else(|| anyhow!("Database name is not available"))?
                    .to_string(),
                host: connect_options.get_host().to_string(),
                port: connect_options.get_port(),
                username: connect_options.get_username().to_string(),
                password: env::var("DATABASE_PASSWORD").ok(),
            },
            ..mock_config()?
        })
    }

    pub fn mock_scheduler_job(
        job_id: Uuid,
        job_type: SchedulerJob,
        schedule: impl Into<String>,
    ) -> RawSchedulerJobStoredData {
        RawSchedulerJobStoredData {
            id: job_id,
            job_type: 3,
            count: Some(0),
            last_tick: None,
            next_tick: Some(12),
            ran: Some(false),
            stopped: Some(false),
            schedule: Some(schedule.into()),
            repeating: None,
            last_updated: None,
            extra: Some(SchedulerJobMetadata::new(job_type).try_into().unwrap()),
            time_offset_seconds: Some(0),
            repeated_every: None,
        }
    }

    #[sqlx::test]
    async fn can_resume_jobs(pool: PgPool) -> anyhow::Result<()> {
        let mock_config = mock_scheduler_config(&pool).await?;
        let user = mock_user()?;
        let api = Arc::new(mock_api_with_config(pool, mock_config).await?);

        let resources_trigger_job_id = uuid!("00000000-0000-0000-0000-000000000001");
        let content_trigger_job_id = uuid!("00000000-0000-0000-0000-000000000002");
        let schedule_job_id = uuid!("00000000-0000-0000-0000-000000000003");
        let notifications_send_job_id = uuid!("00000000-0000-0000-0000-000000000004");

        // Create user, trackers and tracker jobs.
        api.db.upsert_user(user.clone()).await?;
        let resources_tracker = api
            .web_scraping(&user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "tracker-one".to_string(),
                url: "https://localhost:1234/my/app?q=2".parse()?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Default::default(),
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
                url: "https://localhost:1234/my/app?q=2".parse()?,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Default::default(),
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
        api.web_scraping_system()
            .update_web_page_tracker_job(resources_tracker.id, Some(resources_trigger_job_id))
            .await?;
        api.web_scraping_system()
            .update_web_page_tracker_job(content_tracker.id, Some(content_trigger_job_id))
            .await?;

        // Add job registrations.
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                resources_trigger_job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                },
                "1 2 3 4 5 6",
            ),
        )
        .await?;
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                content_trigger_job_id,
                SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                },
                "1 2 3 4 5 6",
            ),
        )
        .await?;
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                schedule_job_id,
                SchedulerJob::WebPageTrackersSchedule,
                "0 * 0 * * *",
            ),
        )
        .await?;
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                notifications_send_job_id,
                SchedulerJob::NotificationsSend,
                "0 * 2 * * *",
            ),
        )
        .await?;

        let mut scheduler = Scheduler::start(api.clone()).await?;

        assert!(
            scheduler
                .inner_scheduler
                .next_tick_for_job(resources_trigger_job_id)
                .await?
                .is_some()
        );

        assert!(
            scheduler
                .inner_scheduler
                .next_tick_for_job(content_trigger_job_id)
                .await?
                .is_some()
        );

        assert!(
            scheduler
                .inner_scheduler
                .next_tick_for_job(schedule_job_id)
                .await?
                .is_some()
        );

        assert!(
            scheduler
                .inner_scheduler
                .next_tick_for_job(notifications_send_job_id)
                .await?
                .is_some()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn schedules_unique_jobs_if_not_started(pool: PgPool) -> anyhow::Result<()> {
        let mock_config = mock_scheduler_config(&pool).await?;
        let api = Arc::new(mock_api_with_config(pool, mock_config).await?);
        Scheduler::start(api.clone()).await?;

        let jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 3);

        let mut jobs = jobs
            .into_iter()
            .map(|job_result| job_result.map(|job| (job.job_type, job.extra, job.schedule)))
            .collect::<anyhow::Result<Vec<(_, _, _)>>>()?;
        jobs.sort_by(|job_a, job_b| job_a.1.cmp(&job_b.1));

        assert_debug_snapshot!(jobs, @r###"
        [
            (
                0,
                Some(
                    [
                        1,
                        0,
                    ],
                ),
                Some(
                    "0 * 0 * * *",
                ),
            ),
            (
                0,
                Some(
                    [
                        2,
                        0,
                    ],
                ),
                Some(
                    "0 * 1 * * *",
                ),
            ),
            (
                0,
                Some(
                    [
                        3,
                        0,
                    ],
                ),
                Some(
                    "0 * 2 * * *",
                ),
            ),
        ]
        "###);

        Ok(())
    }

    #[sqlx::test]
    async fn schedules_unique_jobs_if_cannot_resume(pool: PgPool) -> anyhow::Result<()> {
        let mock_config = mock_scheduler_config(&pool).await?;
        let api = Arc::new(mock_api_with_config(pool, mock_config).await?);

        let schedule_job_id = uuid!("00000000-0000-0000-0000-000000000001");
        let fetch_job_id = uuid!("00000000-0000-0000-0000-000000000002");
        let notifications_send_job_id = uuid!("00000000-0000-0000-0000-000000000003");

        // Add job registration.
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                schedule_job_id,
                SchedulerJob::WebPageTrackersSchedule,
                // Different schedule - every hour, not every minute.
                "0 0 * * * * *",
            ),
        )
        .await?;
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                fetch_job_id,
                SchedulerJob::WebPageTrackersFetch,
                // Different schedule - every day, not every minute.
                "0 0 0 * * * *",
            ),
        )
        .await?;
        mock_upsert_scheduler_job(
            &api.db,
            &mock_scheduler_job(
                notifications_send_job_id,
                SchedulerJob::NotificationsSend,
                // Different schedule - every day, not every minute.
                "0 0 0 * * * *",
            ),
        )
        .await?;

        Scheduler::start(api.clone()).await?;

        // Old jobs should have been removed.
        assert!(
            mock_get_scheduler_job(&api.db, schedule_job_id)
                .await?
                .is_none()
        );
        assert!(
            mock_get_scheduler_job(&api.db, fetch_job_id)
                .await?
                .is_none()
        );
        assert!(
            mock_get_scheduler_job(&api.db, notifications_send_job_id)
                .await?
                .is_none()
        );

        let jobs = api.db.get_scheduler_jobs(10).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 3);

        let mut jobs = jobs
            .into_iter()
            .map(|job_result| job_result.map(|job| (job.job_type, job.extra, job.schedule)))
            .collect::<anyhow::Result<Vec<(_, _, _)>>>()?;
        jobs.sort_by(|job_a, job_b| job_a.1.cmp(&job_b.1));

        assert_debug_snapshot!(jobs, @r###"
        [
            (
                0,
                Some(
                    [
                        1,
                        0,
                    ],
                ),
                Some(
                    "0 * 0 * * *",
                ),
            ),
            (
                0,
                Some(
                    [
                        2,
                        0,
                    ],
                ),
                Some(
                    "0 * 1 * * *",
                ),
            ),
            (
                0,
                Some(
                    [
                        3,
                        0,
                    ],
                ),
                Some(
                    "0 * 2 * * *",
                ),
            ),
        ]
        "###);

        Ok(())
    }
}
