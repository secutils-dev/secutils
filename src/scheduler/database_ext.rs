mod raw_scheduler_job_stored_data;

pub use self::raw_scheduler_job_stored_data::RawSchedulerJobStoredData;

use crate::database::Database;
use async_stream::try_stream;
use futures::Stream;
use sqlx::query_as;
use uuid::Uuid;

/// Extends the primary database with the Scheduler-related methods.
impl Database {
    /// Retrieves the scheduled jobs from the `scheduler_jobs` table.
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
    use crate::database::Database;
    use futures::{Stream, StreamExt};
    use sqlx::{PgPool, query, query_as};
    use uuid::Uuid;

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
                .collect::<Result<Vec<_>, _>>()?,
            (0..=9).map(|n| Some(946720800 + n)).collect::<Vec<_>>()
        );

        Ok(())
    }
}
