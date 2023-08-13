mod raw_scheduler_job_stored_data;

use crate::database::Database;
use anyhow::bail;
use async_stream::try_stream;
use futures::Stream;
use sqlx::{query, query_as, QueryBuilder, Sqlite};
use std::time::Duration;
use time::OffsetDateTime;
use tokio_cron_scheduler::{
    JobAndNextTick, JobId, JobIdAndNotification, JobNotification, JobStoredData, NotificationData,
    NotificationId,
};

use self::raw_scheduler_job_stored_data::RawSchedulerJobStoredData;

/// Extends primary database with the Scheduler-related methods.
impl Database {
    /// Retrieves scheduler job from the `scheduler_jobs` table using Job ID.
    pub async fn get_scheduler_job(&self, id: JobId) -> anyhow::Result<Option<JobStoredData>> {
        let id = id.hyphenated();
        query_as!(
            RawSchedulerJobStoredData,
            r#"
SELECT id as "id: uuid::fmt::Hyphenated", last_updated, next_tick, last_tick, job_type as "job_type!", count,
       ran, stopped, schedule, repeating, repeated_every, extra
FROM scheduler_jobs
WHERE id = ?1
                "#,
            id
        )
            .fetch_optional(&self.pool)
            .await?
            .map(JobStoredData::try_from)
            .transpose()
    }

    /// Upserts scheduler job to the `scheduler_jobs` table.
    pub async fn upsert_scheduler_job(&self, job: &JobStoredData) -> anyhow::Result<()> {
        let raw_job = RawSchedulerJobStoredData::try_from(job)?;

        query!(
            r#"
INSERT INTO scheduler_jobs (id, last_updated, next_tick, job_type, count, ran, stopped, schedule,
                            repeating, repeated_every, extra, last_tick)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
ON CONFLICT(id) DO UPDATE SET last_updated=excluded.last_updated, next_tick=excluded.next_tick,
                            job_type=excluded.job_type, count=excluded.count, ran=excluded.ran,
                            stopped=excluded.stopped, schedule=excluded.schedule,
                            repeating=excluded.repeating, repeated_every=excluded.repeated_every,
                            extra=excluded.extra, last_tick=excluded.last_tick
        "#,
            raw_job.id,
            raw_job.last_updated,
            raw_job.next_tick,
            raw_job.job_type,
            raw_job.count,
            raw_job.ran,
            raw_job.stopped,
            raw_job.schedule,
            raw_job.repeating,
            raw_job.repeated_every,
            raw_job.extra,
            raw_job.last_tick
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Updates `stopped` job value to the `scheduler_jobs` table.
    pub async fn set_scheduler_job_stopped_state(
        &self,
        id: JobId,
        stopped: bool,
    ) -> anyhow::Result<()> {
        let id = id.hyphenated();
        let stopped = stopped as i64;

        query!(
            r#"
UPDATE scheduler_jobs
SET stopped = ?2
WHERE id = ?1
        "#,
            id,
            stopped
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Removes scheduler job from the `scheduler_jobs` table using Job ID.
    pub async fn remove_scheduler_job(&self, id: JobId) -> anyhow::Result<()> {
        let id = id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_jobs
WHERE id = ?1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves the scheduled jobs from `scheduler_jobs` table based on .
    pub fn get_scheduler_jobs(
        &self,
        page_size: usize,
    ) -> impl Stream<Item = anyhow::Result<JobStoredData>> + '_ {
        let page_limit = page_size as i64;
        try_stream! {
            let mut last_id = "".to_string();

            loop {
                 let jobs = query_as!(RawSchedulerJobStoredData,
r#"
SELECT id as "id: uuid::fmt::Hyphenated", last_updated, next_tick, last_tick, job_type as "job_type!", count,
       ran, stopped, schedule, repeating, repeated_every, extra
FROM scheduler_jobs
WHERE id > ?1
ORDER BY id
LIMIT ?2;
"#,
            last_id, page_limit
        )
            .fetch_all(&self.pool)
            .await?;

                let is_last_page = jobs.len() < page_size;
                for job in jobs {
                    last_id = job.id.to_string();
                    yield JobStoredData::try_from(job)?;
                }

                if is_last_page {
                    break;
                }
            }
        }
    }

    /// Retrieves all scheduled jobs from `scheduler_jobs` table that are in a `stopped` state with
    /// the specified value in the `extra` field.
    pub fn get_stopped_scheduler_jobs_by_extra<'a>(
        &'a self,
        page_size: usize,
        extra: &'a [u8],
    ) -> impl Stream<Item = anyhow::Result<JobStoredData>> + '_ {
        let page_limit = page_size as i64;
        try_stream! {
            let mut last_id = "".to_string();

            loop {
                 let jobs = query_as!(RawSchedulerJobStoredData,
r#"
SELECT id as "id: uuid::fmt::Hyphenated", last_updated, next_tick, last_tick, job_type as "job_type!", count,
       ran, stopped, schedule, repeating, repeated_every, extra
FROM scheduler_jobs
WHERE id > ?1 AND stopped = 1 AND extra = ?3
ORDER BY id
LIMIT ?2;
"#,
            last_id, page_limit, extra
        )
            .fetch_all(&self.pool)
            .await?;

                let is_last_page = jobs.len() < page_size;
                for job in jobs {
                    last_id = job.id.to_string();
                    yield JobStoredData::try_from(job)?;
                }

                if is_last_page {
                    break;
                }
            }
        }
    }

    /// Retrieves next scheduled jobs from `scheduler_jobs` table.
    pub async fn get_next_scheduler_jobs(&self) -> anyhow::Result<Vec<JobAndNextTick>> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let jobs = query!(
            r#"
SELECT id as "id: uuid::fmt::Hyphenated", job_type, next_tick, last_tick
FROM scheduler_jobs
WHERE next_tick > 0 AND next_tick < ?1
            "#,
            now
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs
            .into_iter()
            .map(|record| JobAndNextTick {
                id: Some(record.id.into_uuid().into()),
                last_tick: record.last_tick.map(|ts| ts as u64),
                next_tick: record.next_tick.unwrap_or_default() as u64,
                job_type: record.job_type as i32,
            })
            .collect())
    }

    /// Updates scheduler job ticks in the `scheduler_jobs` table.
    pub async fn set_scheduler_job_ticks(
        &self,
        id: JobId,
        next_tick: Option<OffsetDateTime>,
        last_tick: Option<OffsetDateTime>,
    ) -> anyhow::Result<()> {
        let id = id.hyphenated();
        let next_tick = next_tick
            .map(|tick| tick.unix_timestamp())
            .unwrap_or_default();
        let last_tick = last_tick.map(|tick| tick.unix_timestamp());

        query!(
            r#"
UPDATE scheduler_jobs
SET next_tick=?2, last_tick=?3
WHERE id = ?1
        "#,
            id,
            next_tick,
            last_tick
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves time until the next scheduler job from the `scheduler_jobs` table.
    pub async fn get_scheduler_time_until_next_job(
        &self,
        since: OffsetDateTime,
    ) -> anyhow::Result<Option<Duration>> {
        let since = since.unix_timestamp();
        let next_tick = query!(
            r#"
SELECT next_tick
FROM scheduler_jobs
WHERE next_tick > 0 AND next_tick > ?
ORDER BY next_tick ASC
            "#,
            since
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(next_tick.and_then(|next_tick| {
            let next_tick = next_tick.next_tick? as u64;
            if next_tick > 0 {
                Some(Duration::from_secs(next_tick - since as u64))
            } else {
                None
            }
        }))
    }

    /// Retrieves scheduler notification from the `scheduler_notifications` table using Notification ID.
    pub async fn get_scheduler_notification(
        &self,
        id: NotificationId,
    ) -> anyhow::Result<Option<NotificationData>> {
        let id = id.hyphenated();
        let notification = query!(
            r#"
SELECT job_id as "job_id: uuid::fmt::Hyphenated", extra
FROM scheduler_notifications
WHERE id = ?1
                "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        let notification = if let Some(notification) = notification {
            notification
        } else {
            return Ok(None);
        };

        let states = query!(
            r#"
SELECT state
FROM scheduler_notification_states
WHERE id = ?1
            "#,
            id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(NotificationData {
            job_id: Some(JobIdAndNotification {
                job_id: Some(notification.job_id.into_uuid().into()),
                notification_id: Some(id.into_uuid().into()),
            }),
            job_states: states
                .into_iter()
                .map(|record| record.state as i32)
                .collect(),
            extra: notification.extra.unwrap_or_default(),
        }))
    }

    /// Upserts scheduler notification to the `scheduler_notifications` table.
    pub async fn upsert_scheduler_notification(
        &self,
        notification: &NotificationData,
    ) -> anyhow::Result<()> {
        let (job_id, notification_id) = match notification.job_id_and_notification_id_from_data() {
            Some((job_id, notification_id)) => (job_id, notification_id),
            None => {
                bail!(
                    "Job ID and Notification ID are required for scheduler notification upsertion"
                );
            }
        };

        let notification_id = notification_id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_notification_states
WHERE id = ?1
            "#,
            notification_id
        )
        .execute(&self.pool)
        .await?;

        let job_id = job_id.hyphenated();
        query!(
            r#"
INSERT INTO scheduler_notifications (id, job_id, extra)
VALUES (?1, ?2, ?3)
ON CONFLICT(id) DO UPDATE SET job_id=excluded.job_id, extra=excluded.extra
        "#,
            notification_id,
            job_id,
            notification.extra
        )
        .execute(&self.pool)
        .await?;

        if !notification.job_states.is_empty() {
            QueryBuilder::<Sqlite>::new("INSERT INTO scheduler_notification_states (id, state) ")
                .push_values(notification.job_states.iter(), |mut b, state| {
                    b.push_bind(notification_id).push_bind(*state as i64);
                })
                .build()
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    /// Removes scheduler notification from the `scheduler_notifications` table using notification ID.
    pub async fn remove_scheduler_notification(&self, id: NotificationId) -> anyhow::Result<()> {
        let id = id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_notifications
WHERE id = ?1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves notification ids from `scheduler_notifications` table.
    pub async fn get_scheduler_notification_ids_for_job_and_state(
        &self,
        job_id: JobId,
        state: JobNotification,
    ) -> anyhow::Result<Vec<NotificationId>> {
        let job_id = job_id.hyphenated();
        let state = state as i32;
        let notifications = query!(
            r#"
SELECT DISTINCT notifications.id as "id!: uuid::fmt::Hyphenated"
FROM scheduler_notifications as notifications
RIGHT JOIN scheduler_notification_states as states ON notifications.id = states.id
WHERE notifications.job_id = ?1 AND states.state = ?2
            "#,
            job_id,
            state
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(notifications
            .into_iter()
            .map(|notification| notification.id.into_uuid())
            .collect())
    }

    /// Retrieves notification ids from `scheduler_notifications` table.
    pub async fn get_scheduler_notification_ids_for_job(
        &self,
        job_id: JobId,
    ) -> anyhow::Result<Vec<NotificationId>> {
        let job_id = job_id.hyphenated();
        let notifications = query!(
            r#"
SELECT DISTINCT id as "id!: uuid::fmt::Hyphenated"
FROM scheduler_notifications
WHERE job_id = ?1
            "#,
            job_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(notifications
            .into_iter()
            .map(|notification| notification.id.into_uuid())
            .collect())
    }

    /// Removes scheduler notification from the `scheduler_notifications` table using notification ID.
    pub async fn remove_scheduler_notification_for_state(
        &self,
        notification_id: NotificationId,
        state: JobNotification,
    ) -> anyhow::Result<bool> {
        let notification_id = notification_id.hyphenated();
        let state = state as i32;
        let result = query!(
            r#"
DELETE FROM scheduler_notification_states
WHERE id = ?1 AND state = ?2
            "#,
            notification_id,
            state
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Removes scheduler notification from the `scheduler_notifications` table using notification ID.
    pub async fn remove_scheduler_notification_for_job(&self, job_id: JobId) -> anyhow::Result<()> {
        let job_id = job_id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_notifications
WHERE job_id = ?1
            "#,
            job_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::mock_db;
    use futures::{Stream, StreamExt};
    use insta::assert_debug_snapshot;
    use std::time::Duration;
    use time::OffsetDateTime;
    use tokio_cron_scheduler::{
        CronJob, JobIdAndNotification, JobNotification, JobStored, JobStoredData, JobType,
        NonCronJob, NotificationData,
    };
    use uuid::uuid;

    #[actix_rt::test]
    async fn can_add_and_retrieve_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946720800,
                ),
                last_tick: Some(
                    946720700,
                ),
                next_tick: 946720900,
                job_type: 0,
                count: 3,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 0 1 1 * *",
                        },
                    ),
                ),
            },
        )
        "###);

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946820800,
                ),
                last_tick: Some(
                    946820700,
                ),
                next_tick: 946820900,
                job_type: 2,
                count: 0,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    NonCronJob(
                        NonCronJob {
                            repeating: false,
                            repeated_every: 0,
                        },
                    ),
                ),
            },
        )
        "###);

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946720800,
                ),
                last_tick: Some(
                    946720700,
                ),
                next_tick: 946720900,
                job_type: 0,
                count: 3,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 0 1 1 * *",
                        },
                    ),
                ),
            },
        )
        "###);
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946820800,
                ),
                last_tick: Some(
                    946820700,
                ),
                next_tick: 946820900,
                job_type: 2,
                count: 0,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    NonCronJob(
                        NonCronJob {
                            repeating: false,
                            repeated_every: 0,
                        },
                    ),
                ),
            },
        )
        "###);

        db.upsert_scheduler_job(&JobStoredData {
            id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
            last_updated: Some(946721800u64),
            last_tick: Some(946721700u64),
            next_tick: 946721900u64,
            count: 4,
            job_type: JobType::Cron as i32,
            extra: vec![1, 2, 3, 4, 5],
            ran: true,
            stopped: true,
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 0 1 1 * *".to_string(),
            })),
        })
        .await?;

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946721800,
                ),
                last_tick: Some(
                    946721700,
                ),
                next_tick: 946721900,
                job_type: 0,
                count: 4,
                extra: [
                    1,
                    2,
                    3,
                    4,
                    5,
                ],
                ran: true,
                stopped: true,
                job: Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 0 1 1 * *",
                        },
                    ),
                ),
            },
        )
        "###);

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946820800,
                ),
                last_tick: Some(
                    946820700,
                ),
                next_tick: 946820900,
                job_type: 2,
                count: 0,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    NonCronJob(
                        NonCronJob {
                            repeating: false,
                            repeated_every: 0,
                        },
                    ),
                ),
            },
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_job_stopped_state() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let job_one_id = uuid!("00000000-0000-0000-0000-000000000001");
        let job_two_id = uuid!("00000000-0000-0000-0000-000000000002");

        let jobs = vec![
            JobStoredData {
                id: Some(job_one_id.into()),
                last_updated: None,
                last_tick: None,
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(job_two_id.into()),
                last_updated: None,
                last_tick: None,
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        let job_one_stopped = db
            .get_scheduler_job(job_one_id)
            .await?
            .map(|job| job.stopped);
        let job_two_stopped = db
            .get_scheduler_job(job_two_id)
            .await?
            .map(|job| job.stopped);
        assert_eq!(
            (job_one_stopped, job_two_stopped),
            (Some(false), Some(false))
        );

        db.set_scheduler_job_stopped_state(job_one_id, true).await?;

        let job_one_stopped = db
            .get_scheduler_job(job_one_id)
            .await?
            .map(|job| job.stopped);
        let job_two_stopped = db
            .get_scheduler_job(job_two_id)
            .await?
            .map(|job| job.stopped);
        assert_eq!(
            (job_one_stopped, job_two_stopped),
            (Some(true), Some(false))
        );

        db.set_scheduler_job_stopped_state(job_two_id, true).await?;

        let job_one_stopped = db
            .get_scheduler_job(job_one_id)
            .await?
            .map(|job| job.stopped);
        let job_two_stopped = db
            .get_scheduler_job(job_two_id)
            .await?
            .map(|job| job.stopped);
        assert_eq!((job_one_stopped, job_two_stopped), (Some(true), Some(true)));

        db.set_scheduler_job_stopped_state(job_two_id, false)
            .await?;

        let job_one_stopped = db
            .get_scheduler_job(job_one_id)
            .await?
            .map(|job| job.stopped);
        let job_two_stopped = db
            .get_scheduler_job(job_two_id)
            .await?
            .map(|job| job.stopped);
        assert_eq!(
            (job_one_stopped, job_two_stopped),
            (Some(true), Some(false))
        );

        db.set_scheduler_job_stopped_state(job_one_id, false)
            .await?;

        let job_one_stopped = db
            .get_scheduler_job(job_one_id)
            .await?
            .map(|job| job.stopped);
        let job_two_stopped = db
            .get_scheduler_job(job_two_id)
            .await?
            .map(|job| job.stopped);
        assert_eq!(
            (job_one_stopped, job_two_stopped),
            (Some(false), Some(false))
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().id, @r###"
        Some(
            Uuid {
                id1: 7486478208841368175,
                id2: 10540599508476092616,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().id, @r###"
        Some(
            Uuid {
                id1: 64546022934790767,
                id2: 10540599508476092616,
            },
        )
        "###);

        db.remove_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().id, @r###"
        Some(
            Uuid {
                id1: 7486478208841368175,
                id2: 10540599508476092616,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");

        db.remove_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");

        db.remove_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_next_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_next_scheduler_jobs().await?.is_empty());

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_next_scheduler_jobs().await?, @r###"
        [
            JobAndNextTick {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                job_type: 0,
                next_tick: 946720900,
                last_tick: Some(
                    946720700,
                ),
            },
            JobAndNextTick {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                job_type: 2,
                next_tick: 946820900,
                last_tick: Some(
                    946820700,
                ),
            },
        ]
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_job_ticks() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946720900u64);
        assert_eq!(job.last_tick, Some(946720700u64));

        let job = db
            .get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946820900u64);
        assert_eq!(job.last_tick, Some(946820700u64));

        db.set_scheduler_job_ticks(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            None,
            Some(OffsetDateTime::from_unix_timestamp(946720704).unwrap()),
        )
        .await?;

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 0);
        assert_eq!(job.last_tick, Some(946720704u64));

        db.set_scheduler_job_ticks(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            Some(OffsetDateTime::from_unix_timestamp(946720903).unwrap()),
            None,
        )
        .await?;

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946720903);
        assert_eq!(job.last_tick, None);

        db.set_scheduler_job_ticks(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            Some(OffsetDateTime::from_unix_timestamp(946720901).unwrap()),
            Some(OffsetDateTime::from_unix_timestamp(946720702).unwrap()),
        )
        .await?;

        db.set_scheduler_job_ticks(
            uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"),
            Some(OffsetDateTime::from_unix_timestamp(946820901).unwrap()),
            Some(OffsetDateTime::from_unix_timestamp(946820702).unwrap()),
        )
        .await?;

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946720901u64);
        assert_eq!(job.last_tick, Some(946720702u64));

        let job = db
            .get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946820901u64);
        assert_eq!(job.last_tick, Some(946820702u64));

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_scheduler_time_until_next_job() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db
            .get_scheduler_time_until_next_job(OffsetDateTime::now_utc())
            .await?
            .is_none());

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946720800).unwrap()
            )
            .await?,
            Some(Duration::from_secs(100))
        );

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946730900).unwrap()
            )
            .await?,
            Some(Duration::from_secs(90000))
        );

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946820899).unwrap()
            )
            .await?,
            Some(Duration::from_secs(1))
        );

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946820901).unwrap()
            )
            .await?,
            None
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_all_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = db.get_scheduler_jobs(2);
        assert_eq!(jobs.size_hint(), (0, None));
        assert_eq!(jobs.collect::<Vec<_>>().await.len(), 0);

        for n in 0..=9 {
            let job = JobStoredData {
                id: Some(
                    uuid::Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?
                        .into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3, n as u8],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
            };

            db.upsert_scheduler_job(&job).await?;
        }

        let jobs = db.get_scheduler_jobs(2).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 10);

        assert_eq!(
            jobs.into_iter()
                .map(|job| job.map(|job| job.last_updated))
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            (0..=9).map(|n| Some(946720800u64 + n)).collect::<Vec<_>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_all_stopped_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = db
            .get_stopped_scheduler_jobs_by_extra(2, &[1])
            .collect::<Vec<_>>()
            .await;
        assert_eq!(jobs.len(), 0);

        for n in 0..=9 {
            let job = JobStoredData {
                id: Some(
                    uuid::Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?
                        .into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: vec![if n % 3 == 0 { 2 } else { 1 }],
                ran: true,
                stopped: n % 2 == 0,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
            };

            db.upsert_scheduler_job(&job).await?;
        }

        let jobs = db
            .get_stopped_scheduler_jobs_by_extra(2, &[2])
            .collect::<Vec<_>>()
            .await;
        assert_eq!(jobs.len(), 2);
        assert_eq!(
            jobs.into_iter()
                .map(|job| job.map(|job| job.last_updated))
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [0, 6]
                .into_iter()
                .map(|n| Some(946720800u64 + n))
                .collect::<Vec<_>>()
        );

        let jobs = db
            .get_stopped_scheduler_jobs_by_extra(2, &[1])
            .collect::<Vec<_>>()
            .await;
        assert_eq!(jobs.len(), 3);
        assert_eq!(
            jobs.into_iter()
                .map(|job| job.map(|job| job.last_updated))
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [2, 4, 8]
                .into_iter()
                .map(|n| Some(946720800u64 + n))
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_add_and_retrieve_scheduler_notifications() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 568949181200286319,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7486478208841368175,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    1,
                    2,
                ],
                extra: [
                    1,
                    2,
                    3,
                ],
            },
        )
        "###);

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 154618015482200687,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7072147043123282543,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    0,
                    4,
                ],
                extra: [
                    4,
                    5,
                    6,
                ],
            },
        )
        "###);

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("11255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @"None");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_notifications() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        db.upsert_scheduler_notification(&NotificationData {
            job_id: Some(JobIdAndNotification {
                job_id: Some(uuid!("17e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
            }),
            job_states: vec![JobNotification::Removed as i32],
            extra: vec![1, 2, 3, 4, 5],
        })
        .await?;

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 1721870685807133295,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7486478208841368175,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    4,
                ],
                extra: [
                    1,
                    2,
                    3,
                    4,
                    5,
                ],
            },
        )
        "###);

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 154618015482200687,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7072147043123282543,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    0,
                    4,
                ],
                extra: [
                    4,
                    5,
                    6,
                ],
            },
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_scheduler_notifications() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());

        db.remove_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());

        db.remove_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        db.remove_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_notification_ids_for_job_and_state() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Stop as i32,
                ],
                extra: vec![1, 2, 3],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Started)
            .await?, @r###"
        [
            67e55044-10b1-426f-9247-bb680e5fe0c8,
            77e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Scheduled)
            .await?, @r###"
        [
            67e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Stop)
            .await?, @r###"
        [
            77e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Done)
            .await?, @"[]");

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Removed)
            .await?, @r###"
        [
            62255044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("03335044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Removed)
            .await?, @"[]");

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("00000044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Started)
            .await?, @"[]");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_notification_ids_for_job() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Stop as i32,
                ],
                extra: vec![1, 2, 3],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        [
            67e55044-10b1-426f-9247-bb680e5fe0c8,
            77e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        [
            62255044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job(uuid!("00000044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @"[]");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_notifications_for_state() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![JobNotification::Removed as i32],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        db.remove_scheduler_notification_for_state(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Done,
        )
        .await?;
        db.remove_scheduler_notification_for_state(
            uuid!("00055044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Started,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            1,
            2,
        ]
        "###);
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            4,
        ]
        "###);

        db.remove_scheduler_notification_for_state(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Started,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            1,
        ]
        "###);
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            4,
        ]
        "###);

        db.remove_scheduler_notification_for_state(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Scheduled,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @"[]");
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            4,
        ]
        "###);

        db.remove_scheduler_notification_for_state(
            uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Removed,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @"[]");
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @"[]");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_notifications_for_job() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Stop as i32,
                ],
                extra: vec![1, 2, 3],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        db.remove_scheduler_notification_for_job(uuid!("67e00000-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());

        db.remove_scheduler_notification_for_job(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        db.remove_scheduler_notification_for_job(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        db.remove_scheduler_notification_for_job(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        Ok(())
    }
}
