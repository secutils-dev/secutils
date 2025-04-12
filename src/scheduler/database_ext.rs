mod raw_scheduler_job_stored_data;

pub use self::raw_scheduler_job_stored_data::RawSchedulerJobStoredData;

use crate::{database::Database, scheduler::SchedulerJobMetadata};
use anyhow::{anyhow, bail};
use async_stream::try_stream;
use futures::Stream;
use sqlx::{query, query_as};
use uuid::Uuid;

/// Extends primary database with the Scheduler-related methods.
impl Database {
    /// Retrieves scheduler job metadata from the `scheduler_jobs` table using Job ID.
    pub async fn get_scheduler_job_meta(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<SchedulerJobMetadata>> {
        query!(r#"SELECT extra FROM scheduler_jobs WHERE id = $1"#, id)
            .fetch_optional(&self.pool)
            .await?
            .and_then(|record| record.extra)
            .map(|extra| SchedulerJobMetadata::try_from(extra.as_slice()))
            .transpose()
    }

    /// Updates scheduler job metadata in the `scheduler_jobs` table using Job ID.
    pub async fn update_scheduler_job_meta(
        &self,
        id: Uuid,
        meta: SchedulerJobMetadata,
    ) -> anyhow::Result<()> {
        let meta = Vec::try_from(meta)?;
        let result = query!(
            r#"UPDATE scheduler_jobs SET extra = $2 WHERE id = $1"#,
            id,
            meta
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            bail!(format!("A scheduler job ('{id}') doesn't exist."));
        }

        Ok(())
    }

    /// Updates `stopped` job value to the `scheduler_jobs` table.
    pub async fn reset_scheduler_job_state(&self, id: Uuid, stopped: bool) -> anyhow::Result<()> {
        let metadata = self
            .get_scheduler_job_meta(id)
            .await?
            .ok_or_else(|| anyhow!("A scheduler job ('{id}') doesn't exist."))?;

        // Every time the job state is reset, we should reset retry state.
        let metadata = Vec::try_from(SchedulerJobMetadata::new(metadata.job_type))?;
        query!(
            r#"
UPDATE scheduler_jobs
SET stopped = $2, extra = $3
WHERE id = $1
        "#,
            id,
            stopped,
            metadata
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves the scheduled jobs from `scheduler_jobs` table.
    pub fn get_scheduler_jobs(
        &self,
        page_size: usize,
    ) -> impl Stream<Item = anyhow::Result<RawSchedulerJobStoredData>> + '_ {
        let page_limit = page_size as i64;
        try_stream! {
            let mut last_id = Uuid::nil();
            let mut conn = self.pool.acquire().await?;
            loop {
                let jobs = query_as!(RawSchedulerJobStoredData,
                    r#"SELECT * FROM scheduler_jobs WHERE id > $1 ORDER BY id LIMIT $2;"#,
                    last_id, page_limit
                )
                .fetch_all(&mut *conn)
                .await?;

                let is_last_page = jobs.len() < page_size;
                for job in jobs {
                    last_id = job.id;
                    yield job;
                }

                if is_last_page {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    pub use super::RawSchedulerJobStoredData;
    use crate::{
        database::Database,
        scheduler::{SchedulerJob, SchedulerJobMetadata, SchedulerJobRetryState},
    };
    use futures::{Stream, StreamExt};
    use sqlx::{PgPool, query, query_as};
    use time::OffsetDateTime;
    use uuid::{Uuid, uuid};

    pub async fn mock_upsert_scheduler_job(
        db: &Database,
        raw_job: &RawSchedulerJobStoredData,
    ) -> anyhow::Result<()> {
        query!(
                    r#"
        INSERT INTO scheduler_jobs (id, last_updated, next_tick, job_type, count, ran, stopped, schedule,
                                    repeating, repeated_every, extra, last_tick)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
            .execute(&db.pool)
            .await?;

        Ok(())
    }

    pub async fn mock_get_scheduler_job(
        db: &Database,
        id: Uuid,
    ) -> anyhow::Result<Option<RawSchedulerJobStoredData>> {
        Ok(query_as!(
            RawSchedulerJobStoredData,
            r#"
        SELECT id, last_updated, next_tick, last_tick, job_type as "job_type!", count,
               ran, stopped, schedule, repeating, repeated_every, extra, time_offset_seconds
        FROM scheduler_jobs
        WHERE id = $1
                        "#,
            id
        )
        .fetch_optional(&db.pool)
        .await?)
    }

    #[sqlx::test]
    async fn can_reset_scheduler_job_state(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        let job_one_id = uuid!("00000000-0000-0000-0000-000000000001");
        let job_two_id = uuid!("00000000-0000-0000-0000-000000000002");

        let jobs = vec![
            RawSchedulerJobStoredData {
                id: job_one_id,
                last_updated: None,
                last_tick: None,
                next_tick: Some(946720900i64),
                count: Some(3),
                job_type: 3,
                extra: Some(
                    SchedulerJobMetadata {
                        job_type: SchedulerJob::WebPageTrackersSchedule,
                        retry: Some(SchedulerJobRetryState {
                            attempts: 5,
                            next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                        }),
                    }
                    .try_into()?,
                ),
                ran: Some(true),
                stopped: Some(false),
                schedule: None,
                repeating: None,
                time_offset_seconds: Some(0),
                repeated_every: None,
            },
            RawSchedulerJobStoredData {
                id: job_two_id,
                last_updated: None,
                last_tick: None,
                next_tick: Some(946820900),
                count: Some(0),
                job_type: 1,
                extra: Some(
                    SchedulerJobMetadata {
                        job_type: SchedulerJob::WebPageTrackersSchedule,
                        retry: None,
                    }
                    .try_into()?,
                ),
                ran: Some(true),
                stopped: Some(false),
                schedule: None,
                repeating: None,
                time_offset_seconds: Some(0),
                repeated_every: None,
            },
        ];

        for job in jobs {
            mock_upsert_scheduler_job(&db, &job).await?;
        }

        let job_one = mock_get_scheduler_job(&db, job_one_id).await?.unwrap();
        assert!(!job_one.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_one.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            }
        );

        let job_two = mock_get_scheduler_job(&db, job_two_id).await?.unwrap();
        assert!(!job_two.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_two.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None
            }
        );

        db.reset_scheduler_job_state(job_one_id, true).await?;

        let job_one = mock_get_scheduler_job(&db, job_one_id).await?.unwrap();
        assert!(job_one.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_one.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None,
            }
        );

        let job_two = mock_get_scheduler_job(&db, job_two_id).await?.unwrap();
        assert!(!job_two.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_two.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None
            }
        );

        db.reset_scheduler_job_state(job_two_id, true).await?;

        let job_one = mock_get_scheduler_job(&db, job_one_id).await?.unwrap();
        assert!(job_one.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_one.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None,
            }
        );

        let job_two = mock_get_scheduler_job(&db, job_two_id).await?.unwrap();
        assert!(job_two.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_two.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None
            }
        );

        db.update_scheduler_job_meta(
            job_one_id,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            },
        )
        .await?;
        db.update_scheduler_job_meta(
            job_two_id,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 10,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            },
        )
        .await?;

        let job_one = mock_get_scheduler_job(&db, job_one_id).await?.unwrap();
        assert!(job_one.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_one.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            }
        );

        let job_two = mock_get_scheduler_job(&db, job_two_id).await?.unwrap();
        assert!(job_two.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_two.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 10,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            }
        );

        db.reset_scheduler_job_state(job_two_id, false).await?;

        let job_one = mock_get_scheduler_job(&db, job_one_id).await?.unwrap();
        assert!(job_one.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_one.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            }
        );

        let job_two = mock_get_scheduler_job(&db, job_two_id).await?.unwrap();
        assert!(!job_two.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_two.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None,
            }
        );

        db.reset_scheduler_job_state(job_one_id, false).await?;

        let job_one = mock_get_scheduler_job(&db, job_one_id).await?.unwrap();
        assert!(!job_one.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_one.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None,
            }
        );

        let job_two = mock_get_scheduler_job(&db, job_two_id).await?.unwrap();
        assert!(!job_two.stopped.unwrap());
        assert_eq!(
            SchedulerJobMetadata::try_from(job_two.extra.unwrap().as_slice())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None,
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_and_retrieve_scheduler_job_metadata(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let jobs = vec![
            RawSchedulerJobStoredData {
                id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                last_updated: Some(946720800),
                last_tick: Some(946720700),
                next_tick: Some(946720900),
                count: Some(3),
                job_type: 3,
                extra: Some(Vec::try_from(SchedulerJobMetadata::new(
                    SchedulerJob::NotificationsSend,
                ))?),
                ran: Some(true),
                stopped: Some(false),
                schedule: None,
                repeating: None,
                time_offset_seconds: Some(0),
                repeated_every: None,
            },
            RawSchedulerJobStoredData {
                id: uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"),
                last_updated: Some(946820800),
                last_tick: Some(946820700),
                next_tick: Some(946820900),
                count: Some(0),
                job_type: 1,
                extra: Some(Vec::try_from(SchedulerJobMetadata {
                    job_type: SchedulerJob::WebPageTrackersSchedule,
                    retry: Some(SchedulerJobRetryState {
                        attempts: 5,
                        next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    }),
                })?),
                ran: Some(true),
                stopped: Some(false),
                schedule: None,
                repeating: None,
                time_offset_seconds: Some(0),
                repeated_every: None,
            },
        ];

        for job in jobs {
            mock_upsert_scheduler_job(&db, &job).await?;
        }

        assert_eq!(
            db.get_scheduler_job_meta(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
                .await?
                .unwrap(),
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend)
        );
        assert_eq!(
            db.get_scheduler_job_meta(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
                .await?
                .unwrap(),
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            }
        );

        db.update_scheduler_job_meta(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            },
        )
        .await?;
        db.update_scheduler_job_meta(
            uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"),
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend),
        )
        .await?;

        assert_eq!(
            db.get_scheduler_job_meta(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
                .await?
                .unwrap(),
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 5,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                }),
            }
        );
        assert_eq!(
            db.get_scheduler_job_meta(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
                .await?
                .unwrap(),
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend)
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_all_jobs(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        let jobs = db.get_scheduler_jobs(2);
        assert_eq!(jobs.size_hint(), (0, None));
        assert_eq!(jobs.collect::<Vec<_>>().await.len(), 0);

        for n in 0..=9 {
            let job = RawSchedulerJobStoredData {
                id: Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?,
                last_updated: Some(946720800 + n),
                last_tick: Some(946720700),
                next_tick: Some(946720900),
                count: Some(n as i32),
                job_type: 3,
                extra: Some(vec![1, 2, 3, n as u8]),
                ran: Some(true),
                stopped: Some(false),
                schedule: None,
                repeating: None,
                time_offset_seconds: Some(0),
                repeated_every: None,
            };

            mock_upsert_scheduler_job(&db, &job).await?;
        }

        let jobs = db.get_scheduler_jobs(2).collect::<Vec<_>>().await;
        assert_eq!(jobs.len(), 10);

        assert_eq!(
            jobs.into_iter()
                .map(|job| job.map(|job| job.last_updated))
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            (0..=9).map(|n| Some(946720800 + n)).collect::<Vec<_>>()
        );

        Ok(())
    }
}
