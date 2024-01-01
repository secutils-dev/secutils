mod raw_web_page_data_revision;
mod raw_web_page_tracker;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    scheduler::SchedulerJobMetadata,
    users::UserId,
    utils::web_scraping::{
        database_ext::raw_web_page_data_revision::RawWebPageDataRevision, WebPageDataRevision,
        WebPageTracker, WebPageTrackerTag,
    },
};
use anyhow::{anyhow, bail};
use async_stream::try_stream;
use futures::Stream;
use raw_web_page_tracker::RawWebPageTracker;
use sqlx::{error::ErrorKind as SqlxErrorKind, query, query_as, Pool, Sqlite};
use time::OffsetDateTime;
use uuid::Uuid;

/// A database extension for the web scraping utility-related operations.
pub struct WebScrapingDatabaseExt<'pool> {
    pool: &'pool Pool<Sqlite>,
    user_id: UserId,
}

impl<'pool> WebScrapingDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Sqlite>, user_id: UserId) -> Self {
        Self { pool, user_id }
    }

    /// Retrieves all web page trackers of the specified kind.
    pub async fn get_web_page_trackers<Tag: WebPageTrackerTag>(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<Tag>>> {
        let kind = Vec::try_from(Tag::KIND)?;
        let raw_trackers = query_as!(
            RawWebPageTracker,
            r#"
SELECT id, name, url, kind, job_id, job_config, user_id, data, created_at
FROM user_data_web_scraping_trackers
WHERE user_id = ?1 AND kind = ?2
ORDER BY created_at
                "#,
            *self.user_id,
            kind
        )
        .fetch_all(self.pool)
        .await?;

        let mut trackers = vec![];
        for raw_tracker in raw_trackers {
            trackers.push(WebPageTracker::try_from(raw_tracker)?);
        }

        Ok(trackers)
    }

    /// Retrieves web page tracker for the specified user with the specified ID.
    pub async fn get_web_page_tracker<Tag: WebPageTrackerTag>(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<Tag>>> {
        let kind = Vec::try_from(Tag::KIND)?;
        let id = id.as_ref();
        query_as!(
            RawWebPageTracker,
            r#"
    SELECT id, name, url, kind, user_id, job_id, job_config, data, created_at
    FROM user_data_web_scraping_trackers
    WHERE user_id = ?1 AND id = ?2 AND kind = ?3
                    "#,
            *self.user_id,
            id,
            kind
        )
        .fetch_optional(self.pool)
        .await?
        .map(WebPageTracker::try_from)
        .transpose()
    }

    /// Inserts web page tracker.
    pub async fn insert_web_page_tracker<Tag: WebPageTrackerTag>(
        &self,
        tracker: &WebPageTracker<Tag>,
    ) -> anyhow::Result<()> {
        let raw_tracker = RawWebPageTracker::try_from(tracker)?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_scraping_trackers (user_id, id, name, url, kind, job_id, job_config, data, created_at)
    VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9 )
            "#,
            *self.user_id,
            raw_tracker.id,
            raw_tracker.name,
            raw_tracker.url,
            raw_tracker.kind,
            raw_tracker.job_id,
            raw_tracker.job_config,
            raw_tracker.data,
            raw_tracker.created_at
        )
        .execute(self.pool)
        .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Web page tracker ('{}') already exists.",
                    tracker.name
                )))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create web page tracker ('{}') due to unknown reason.",
                    tracker.name
                )))
            });
        }

        Ok(())
    }

    /// Updates web page tracker.
    pub async fn update_web_page_tracker<Tag: WebPageTrackerTag>(
        &self,
        tracker: &WebPageTracker<Tag>,
    ) -> anyhow::Result<()> {
        let kind = Vec::try_from(Tag::KIND)?;
        let raw_tracker = RawWebPageTracker::try_from(tracker)?;
        let result = query!(
            r#"
UPDATE user_data_web_scraping_trackers
SET name = ?4, url = ?5, job_config = ?6, data = ?7, job_id = ?8
WHERE user_id = ?1 AND id = ?2 AND kind = ?3
        "#,
            *self.user_id,
            raw_tracker.id,
            kind,
            raw_tracker.name,
            raw_tracker.url,
            raw_tracker.job_config,
            raw_tracker.data,
            raw_tracker.job_id
        )
        .execute(self.pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "A web page tracker ('{}') doesn't exist.",
                        tracker.name
                    )));
                }
            }
            Err(err) => {
                let is_conflict_error = err
                    .as_database_error()
                    .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                    .unwrap_or_default();
                bail!(if is_conflict_error {
                    SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                        "Web page tracker ('{}') already exists.",
                        tracker.name
                    )))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update web page tracker ('{}') due to unknown reason.",
                        tracker.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes web page tracker for the specified user with the specified ID.
    pub async fn remove_web_page_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        let id = id.as_ref();
        query!(
            r#"
    DELETE FROM user_data_web_scraping_trackers
    WHERE user_id = ?1 AND id = ?2
                    "#,
            *self.user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all tracked revisions for the specified web page tracker.
    pub async fn get_web_page_tracker_history<Tag: WebPageTrackerTag>(
        &self,
        tracker_id: Uuid,
    ) -> anyhow::Result<Vec<WebPageDataRevision<Tag>>> {
        let kind = Vec::try_from(Tag::KIND)?;
        let raw_revisions = query_as!(
            RawWebPageDataRevision,
            r#"
SELECT history.id, history.tracker_id, history.data, history.created_at
FROM user_data_web_scraping_trackers_history as history
INNER JOIN user_data_web_scraping_trackers as trackers
ON history.tracker_id = trackers.id
WHERE history.user_id = ?1 AND history.tracker_id = ?2 AND trackers.kind = ?3
ORDER BY history.created_at
                "#,
            *self.user_id,
            tracker_id,
            kind
        )
        .fetch_all(self.pool)
        .await?;

        let mut revisions = vec![];
        for raw_revision in raw_revisions {
            revisions.push(WebPageDataRevision::try_from(raw_revision)?);
        }

        Ok(revisions)
    }

    /// Removes web page tracker history.
    pub async fn clear_web_page_tracker_history(&self, tracker_id: Uuid) -> anyhow::Result<()> {
        let id = tracker_id.as_ref();
        query!(
            r#"
    DELETE FROM user_data_web_scraping_trackers_history
    WHERE user_id = ?1 AND tracker_id = ?2
                    "#,
            *self.user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    // Inserts web page tracker revision.
    pub async fn insert_web_page_tracker_history_revision<Tag: WebPageTrackerTag>(
        &self,
        revision: &WebPageDataRevision<Tag>,
    ) -> anyhow::Result<()> {
        let raw_revision = RawWebPageDataRevision::try_from(revision)?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_scraping_trackers_history (user_id, id, tracker_id, data, created_at)
    VALUES ( ?1, ?2, ?3, ?4, ?5 )
            "#,
            *self.user_id,
            raw_revision.id,
            raw_revision.tracker_id,
            raw_revision.data,
            raw_revision.created_at
        )
        .execute(self.pool)
        .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Web page tracker revision ('{}') already exists.",
                    revision.id
                )))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create web page tracker revision ('{}') due to unknown reason.",
                    revision.id
                )))
            });
        }

        Ok(())
    }

    /// Removes web page tracker history.
    pub async fn remove_web_page_tracker_history_revision(
        &self,
        tracker_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<()> {
        query!(
            r#"
    DELETE FROM user_data_web_scraping_trackers_history
    WHERE user_id = ?1 AND tracker_id = ?2 AND id = ?3
                    "#,
            *self.user_id,
            tracker_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

/// A database extension for the web scraping utility-related operations performed on behalf of the
/// system/background jobs.
pub struct WebScrapingDatabaseSystemExt<'pool> {
    pool: &'pool Pool<Sqlite>,
}

impl<'pool> WebScrapingDatabaseSystemExt<'pool> {
    pub fn new(pool: &'pool Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Retrieves all web page trackers that need to be scheduled.
    pub async fn get_unscheduled_web_page_trackers<Tag: WebPageTrackerTag>(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<Tag>>> {
        let kind = Vec::try_from(Tag::KIND)?;
        let raw_trackers = query_as!(
            RawWebPageTracker,
            r#"
SELECT id, name, url, kind, user_id, job_id, job_config, data, created_at
FROM user_data_web_scraping_trackers
WHERE job_config IS NOT NULL AND job_id IS NULL AND kind = ?1
ORDER BY created_at
                "#,
            kind
        )
        .fetch_all(self.pool)
        .await?;

        let mut trackers = vec![];
        for raw_tracker in raw_trackers {
            let tracker = WebPageTracker::try_from(raw_tracker)?;
            // Tracker without revisions shouldn't be scheduled.
            if tracker.settings.revisions > 0 {
                trackers.push(tracker);
            }
        }

        Ok(trackers)
    }

    /// Retrieves all scheduled jobs from `scheduler_jobs` table that are in a `stopped` state.
    pub fn get_pending_web_page_trackers<'a, Tag: WebPageTrackerTag + 'a>(
        &'a self,
        page_size: usize,
    ) -> impl Stream<Item = anyhow::Result<WebPageTracker<Tag>>> + '_ {
        let page_limit = page_size as i64;
        try_stream! {
            let mut last_created_at = 0;
            let kind = Vec::try_from(Tag::KIND)?;
            loop {
                 let records = query!(
r#"
SELECT trackers.id, trackers.name, trackers.url, trackers.kind, trackers.job_id, 
       trackers.job_config, trackers.user_id, trackers.data, trackers.created_at, jobs.extra
FROM user_data_web_scraping_trackers as trackers
INNER JOIN scheduler_jobs as jobs
ON trackers.job_id = jobs.id
WHERE trackers.kind = ?1 AND jobs.stopped = 1 AND trackers.created_at > ?2
ORDER BY trackers.created_at
LIMIT ?3;
"#,
             kind, last_created_at, page_limit
        )
            .fetch_all(self.pool)
            .await?;

                let is_last_page = records.len() < page_size;
                let now = OffsetDateTime::now_utc();
                for record in records {
                    last_created_at = record.created_at;

                    // Check if the tracker job is pending the retry attempt.
                    let job_meta = record.extra.map(|extra| SchedulerJobMetadata::try_from(extra.as_slice())).transpose()?;
                    if let Some(SchedulerJobMetadata { retry: Some(retry), .. }) = job_meta {
                        if retry.next_at > now {
                            continue;
                        }
                    }

                    yield WebPageTracker::<Tag>::try_from(RawWebPageTracker {
                        id: record.id,
                        name: record.name,
                        url: record.url,
                        kind: record.kind,
                        job_id: record.job_id,
                        job_config: record.job_config,
                        user_id: record.user_id,
                        data: record.data,
                        created_at: record.created_at,
                    })?;
                }

                if is_last_page {
                    break;
                }
            }
        }
    }

    /// Retrieves web page tracker by the specified job ID.
    pub async fn get_web_page_tracker_by_job_id<Tag: WebPageTrackerTag>(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<Tag>>> {
        let kind = Vec::try_from(Tag::KIND)?;
        query_as!(
            RawWebPageTracker,
            r#"
    SELECT id, name, url, kind, user_id, job_id, job_config, data, created_at
    FROM user_data_web_scraping_trackers
    WHERE job_id = ?1 AND kind = ?2
                    "#,
            job_id,
            kind
        )
        .fetch_optional(self.pool)
        .await?
        .map(WebPageTracker::try_from)
        .transpose()
    }

    /// Inserts web page tracker.
    pub async fn update_web_page_tracker_job(
        &self,
        id: Uuid,
        job_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        let result = query!(
            r#"
    UPDATE user_data_web_scraping_trackers
    SET job_id = ?2
    WHERE id = ?1
            "#,
            id,
            job_id
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            bail!(SecutilsError::client(format!(
                "A web page tracker ('{id}') doesn't exist.",
            )));
        }

        Ok(())
    }
}

impl Database {
    /// Returns a database extension for the web scraping utility-related operations performed on
    /// behalf of the specified user.
    pub fn web_scraping(&self, user_id: UserId) -> WebScrapingDatabaseExt {
        WebScrapingDatabaseExt::new(&self.pool, user_id)
    }

    /// Returns a database extension for the web scraping utility-related operations performed on
    /// behalf of the system. This extension SHOULD NOT be used by the end-user triggered actions.
    pub fn web_scraping_system(&self) -> WebScrapingDatabaseSystemExt {
        WebScrapingDatabaseSystemExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        scheduler::{
            SchedulerJob, SchedulerJobConfig, SchedulerJobMetadata, SchedulerJobRetryState,
            SchedulerJobRetryStrategy,
        },
        tests::{mock_db, mock_user, MockWebPageTrackerBuilder},
        utils::web_scraping::{
            WebPageContentTrackerTag, WebPageDataRevision, WebPageResource, WebPageResourceContent,
            WebPageResourceContentData, WebPageResourcesData, WebPageResourcesTrackerTag,
            WebPageTracker, WebPageTrackerKind,
        },
    };
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;
    use tokio_cron_scheduler::{CronJob, JobStored, JobStoredData, JobType};
    use url::Url;
    use uuid::{uuid, Uuid};

    fn create_resources_revision(
        id: Uuid,
        tracker_id: Uuid,
        time_shift: i64,
    ) -> anyhow::Result<WebPageDataRevision<WebPageResourcesTrackerTag>> {
        Ok(WebPageDataRevision {
            id,
            tracker_id,
            created_at: OffsetDateTime::from_unix_timestamp(946720800 + time_shift)?,
            data: WebPageResourcesData {
                scripts: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123,
                    }),
                    diff_status: None,
                }],
                styles: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                        size: 321,
                    }),
                    diff_status: None,
                }],
            },
        })
    }

    #[tokio::test]
    async fn can_add_and_retrieve_web_page_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut resources_trackers: Vec<WebPageTracker<WebPageResourcesTrackerTag>> = vec![
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .with_job_config(SchedulerJobConfig {
                schedule: "* * * * *".to_string(),
                retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                    interval: Duration::from_secs(120),
                    max_attempts: 5,
                }),
                notifications: true,
            })
            .build(),
        ];

        let mut content_trackers: Vec<WebPageTracker<WebPageContentTrackerTag>> = vec![
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000004"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in resources_trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }
        for tracker in content_trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_web_page_tracker(resources_trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, resources_trackers.remove(0));

        let tracker = web_scraping
            .get_web_page_tracker(resources_trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, resources_trackers.remove(0));

        let tracker = web_scraping
            .get_web_page_tracker(content_trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, content_trackers.remove(0));

        let tracker = web_scraping
            .get_web_page_tracker(content_trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, content_trackers.remove(0));

        assert!(web_scraping
            .get_web_page_tracker::<WebPageResourcesTrackerTag>(uuid!(
                "00000000-0000-0000-0000-000000000005"
            ))
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_duplicated_web_page_trackers_on_insert() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_web_page_tracker(&tracker).await?;

        let insert_error = web_scraping
            .insert_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            insert_error,
            @r###"
        Error {
            context: "Web page tracker (\'some-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_scraping_trackers.name, user_data_web_scraping_trackers.kind, user_data_web_scraping_trackers.user_id",
                },
            ),
        }
        "###
        );

        // Tracker with the same name, but different kind should be allowed.
        let insert_result = web_scraping
            .insert_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await;
        assert!(insert_result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn can_update_web_page_tracker() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        web_scraping
            .insert_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await?;
        web_scraping
            .insert_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await?;

        web_scraping
            .update_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    "https://secutils.dev",
                    5,
                )?
                .build(),
            )
            .await?;
        web_scraping
            .update_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name-2",
                    "https://secutils.dev",
                    5,
                )?
                .build(),
            )
            .await?;

        let tracker = web_scraping
            .get_web_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            tracker,
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name-2",
                "https://secutils.dev",
                5,
            )?
            .build()
        );

        let tracker = web_scraping
            .get_web_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(
            tracker,
            MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                5,
            )?
            .build()
        );

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_duplicated_resources_trackers_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        let resources_tracker_a = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .build();
        web_scraping
            .insert_web_page_tracker(&resources_tracker_a)
            .await?;

        let resources_tracker_b = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "some-name-2",
            "https://secutils.dev",
            3,
        )?
        .build();
        web_scraping
            .insert_web_page_tracker(&resources_tracker_b)
            .await?;

        let content_tracker_a = MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .build();
        web_scraping
            .insert_web_page_tracker(&content_tracker_a)
            .await?;

        let content_tracker_b = MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000004"),
            "some-name-2",
            "https://secutils.dev",
            3,
        )?
        .build();
        web_scraping
            .insert_web_page_tracker(&content_tracker_b)
            .await?;

        let update_error = web_scraping
            .update_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###"
        Error {
            context: "Web page tracker (\'some-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_scraping_trackers.name, user_data_web_scraping_trackers.kind, user_data_web_scraping_trackers.user_id",
                },
            ),
        }
        "###
        );

        let update_error = web_scraping
            .update_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000004"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###"
        Error {
            context: "Web page tracker (\'some-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_scraping_trackers.name, user_data_web_scraping_trackers.kind, user_data_web_scraping_trackers.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_non_existent_web_page_trackers_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let update_error = db
            .web_scraping(user.id)
            .update_web_page_tracker(
                &MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    "https://secutils.dev",
                    5,
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""A web page tracker ('some-name-2') doesn't exist.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_remove_web_page_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut trackers = vec![
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_web_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker_2 = web_scraping
            .get_web_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker_2, trackers[0].clone());

        web_scraping
            .remove_web_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let tracker = web_scraping
            .get_web_page_tracker::<WebPageResourcesTrackerTag>(uuid!(
                "00000000-0000-0000-0000-000000000001"
            ))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_web_page_tracker::<WebPageResourcesTrackerTag>(uuid!(
                "00000000-0000-0000-0000-000000000002"
            ))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        web_scraping
            .remove_web_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let tracker = web_scraping
            .get_web_page_tracker::<WebPageResourcesTrackerTag>(uuid!(
                "00000000-0000-0000-0000-000000000001"
            ))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_web_page_tracker::<WebPageResourcesTrackerTag>(uuid!(
                "00000000-0000-0000-0000-000000000002"
            ))
            .await?;
        assert!(tracker.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_retrieve_all_web_page_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let resources_trackers = vec![
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let content_trackers = vec![
            MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000004"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in resources_trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }
        for tracker in content_trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        assert_eq!(
            web_scraping.get_web_page_trackers().await?,
            resources_trackers
        );

        assert_eq!(
            web_scraping.get_web_page_trackers().await?,
            content_trackers
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_add_and_retrieve_history_revisions() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        // No history yet.
        for tracker in trackers.iter() {
            assert!(web_scraping
                .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(tracker.id)
                .await?
                .is_empty());
        }

        let mut revisions = vec![
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000001"),
                trackers[0].id,
                0,
            )?,
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000002"),
                trackers[0].id,
                1,
            )?,
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000003"),
                trackers[1].id,
                0,
            )?,
        ];
        for revision in revisions.iter() {
            web_scraping
                .insert_web_page_tracker_history_revision(revision)
                .await?;
        }

        let history = web_scraping
            .get_web_page_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions.remove(0), revisions.remove(0)]);

        let history = web_scraping
            .get_web_page_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions.remove(0)]);

        assert!(web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(uuid!(
                "00000000-0000-0000-0000-000000000004"
            ))
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn can_remove_history_revisions() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        let revisions = vec![
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000001"),
                trackers[0].id,
                0,
            )?,
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000002"),
                trackers[0].id,
                1,
            )?,
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000003"),
                trackers[1].id,
                0,
            )?,
        ];
        for revision in revisions.iter() {
            web_scraping
                .insert_web_page_tracker_history_revision(revision)
                .await?;
        }

        let history = web_scraping
            .get_web_page_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions[0].clone(), revisions[1].clone()]);

        let history = web_scraping
            .get_web_page_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions[2].clone()]);

        // Remove one revision.
        web_scraping
            .remove_web_page_tracker_history_revision(trackers[0].id, revisions[0].id)
            .await?;

        let history = web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions[1].clone()]);

        let history = web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions[2].clone()]);

        // Remove the rest of revisions.
        web_scraping
            .remove_web_page_tracker_history_revision(trackers[0].id, revisions[1].id)
            .await?;
        web_scraping
            .remove_web_page_tracker_history_revision(trackers[1].id, revisions[2].id)
            .await?;

        assert!(web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(trackers[0].id)
            .await?
            .is_empty());
        assert!(web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(trackers[1].id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn can_clear_all_history_revisions_at_once() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        let revisions = vec![
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000001"),
                trackers[0].id,
                0,
            )?,
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000002"),
                trackers[0].id,
                1,
            )?,
            create_resources_revision(
                uuid!("00000000-0000-0000-0000-000000000003"),
                trackers[1].id,
                0,
            )?,
        ];
        for revision in revisions.iter() {
            web_scraping
                .insert_web_page_tracker_history_revision(revision)
                .await?;
        }

        let history = web_scraping
            .get_web_page_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions[0].clone(), revisions[1].clone()]);

        let history = web_scraping
            .get_web_page_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions[2].clone()]);

        // Clear all revisions.
        web_scraping
            .clear_web_page_tracker_history(trackers[0].id)
            .await?;
        web_scraping
            .clear_web_page_tracker_history(trackers[1].id)
            .await?;

        assert!(web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(trackers[0].id)
            .await?
            .is_empty());
        assert!(web_scraping
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(trackers[1].id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn can_retrieve_all_unscheduled_web_page_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let resources_trackers: Vec<WebPageTracker<WebPageResourcesTrackerTag>> = vec![
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name-3",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000004"),
                "some-name-4",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000005"),
                "some-name-5",
                "https://secutils.dev",
                0,
            )?
            .with_schedule("* * * * *")
            .build(),
        ];

        let content_trackers: Vec<WebPageTracker<WebPageContentTrackerTag>> = vec![
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000006"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000007"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000008"),
                "some-name-3",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000009"),
                "some-name-4",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000010"),
                "some-name-5",
                "https://secutils.dev",
                0,
            )?
            .with_schedule("* * * * *")
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in resources_trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }
        for tracker in content_trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        assert_eq!(
            web_scraping.get_web_page_trackers().await?,
            resources_trackers
        );
        assert_eq!(
            web_scraping.get_web_page_trackers().await?,
            content_trackers
        );

        let web_scraping_system = db.web_scraping_system();
        assert_eq!(
            web_scraping_system
                .get_unscheduled_web_page_trackers()
                .await?,
            vec![
                resources_trackers[0].clone(),
                resources_trackers[1].clone(),
                resources_trackers[2].clone()
            ]
        );
        assert_eq!(
            web_scraping_system
                .get_unscheduled_web_page_trackers()
                .await?,
            vec![
                content_trackers[0].clone(),
                content_trackers[1].clone(),
                content_trackers[2].clone()
            ]
        );

        web_scraping_system
            .update_web_page_tracker_job(
                resources_trackers[1].id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        web_scraping_system
            .update_web_page_tracker_job(
                content_trackers[1].id,
                Some(uuid!("00000000-0000-0000-0000-000000000002")),
            )
            .await?;
        assert_eq!(
            web_scraping.get_web_page_trackers().await?,
            vec![
                resources_trackers[0].clone(),
                WebPageTracker {
                    job_id: Some(uuid!("00000000-0000-0000-0000-000000000001")),
                    ..resources_trackers[1].clone()
                },
                resources_trackers[2].clone(),
                resources_trackers[3].clone(),
                resources_trackers[4].clone(),
            ]
        );
        assert_eq!(
            web_scraping.get_web_page_trackers().await?,
            vec![
                content_trackers[0].clone(),
                WebPageTracker {
                    job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                    ..content_trackers[1].clone()
                },
                content_trackers[2].clone(),
                content_trackers[3].clone(),
                content_trackers[4].clone(),
            ]
        );
        assert_eq!(
            web_scraping_system
                .get_unscheduled_web_page_trackers()
                .await?,
            vec![content_trackers[0].clone(), content_trackers[2].clone()]
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_retrieve_web_page_tracker_by_job_id() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers: Vec<WebPageTracker<WebPageResourcesTrackerTag>> = vec![
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .with_job_id(uuid!("00000000-0000-0000-0000-000000000011"))
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .with_job_id(uuid!("00000000-0000-0000-0000-000000000022"))
            .build(),
            MockWebPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name-3",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_web_page_tracker(tracker).await?;
        }

        let web_scraping_system = db.web_scraping_system();
        assert_eq!(
            web_scraping_system
                .get_web_page_tracker_by_job_id(uuid!("00000000-0000-0000-0000-000000000011"))
                .await?,
            Some(trackers[0].clone())
        );
        assert_eq!(
            web_scraping_system
                .get_web_page_tracker_by_job_id(uuid!("00000000-0000-0000-0000-000000000022"))
                .await?,
            Some(trackers[1].clone())
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_update_web_page_trackers_job_id() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .with_schedule("* * * * *")
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_web_page_tracker(&tracker).await?;

        assert_eq!(
            web_scraping
                .get_web_page_tracker::<WebPageResourcesTrackerTag>(tracker.id)
                .await?
                .unwrap()
                .job_id,
            None
        );

        let web_scraping_system = db.web_scraping_system();
        web_scraping_system
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000011")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_web_page_tracker::<WebPageResourcesTrackerTag>(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000011"))
        );

        web_scraping_system
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000022")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_web_page_tracker::<WebPageResourcesTrackerTag>(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000022"))
        );

        Ok(())
    }

    #[tokio::test]
    async fn fails_to_update_web_page_trackers_job_id_if_needed() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .build();

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Non-existent tracker
        let update_result = update_and_fail(
            db.web_scraping_system()
                .update_web_page_tracker_job(
                    tracker.id,
                    Some(uuid!("00000000-0000-0000-0000-000000000011")),
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!("A web page tracker ('{}') doesn't exist.", tracker.id)
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_return_tracker_with_pending_jobs() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageContentTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        let pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageResourcesTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        for n in 0..=2 {
            let job = JobStoredData {
                id: Some(
                    Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?.into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                })
                .try_into()?,
                ran: true,
                stopped: n != 1,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
            };

            db.upsert_scheduler_job(&job).await?;
        }

        for n in 0..=2 {
            let job = JobStoredData {
                id: Some(
                    Uuid::parse_str(&format!("68e55044-10b1-426f-9247-bb680e5fe0c{}", n))?.into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                })
                .try_into()?,
                ran: true,
                stopped: n != 1,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
            };

            db.upsert_scheduler_job(&job).await?;
        }

        for n in 0..=2 {
            db.web_scraping(user.id)
                .insert_web_page_tracker(
                    &MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                        Uuid::parse_str(&format!("77e55044-10b1-426f-9247-bb680e5fe0c{}", n))?,
                        format!("name_{}", n),
                        "https://secutils.dev",
                        3,
                    )?
                    .with_schedule("0 0 * * * *")
                    .with_job_id(Uuid::parse_str(&format!(
                        "67e55044-10b1-426f-9247-bb680e5fe0c{}",
                        n
                    ))?)
                    .build(),
                )
                .await?;
        }

        for n in 0..=2 {
            db.web_scraping(user.id)
                .insert_web_page_tracker(
                    &MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
                        Uuid::parse_str(&format!("78e55044-10b1-426f-9247-bb680e5fe0c{}", n))?,
                        format!("name_{}", n),
                        "https://secutils.dev",
                        3,
                    )?
                    .with_schedule("0 0 * * * *")
                    .with_job_id(Uuid::parse_str(&format!(
                        "68e55044-10b1-426f-9247-bb680e5fe0c{}",
                        n
                    ))?)
                    .build(),
                )
                .await?;
        }

        let pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageContentTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(pending_trackers.len(), 2);

        let pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageResourcesTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(pending_trackers.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn can_return_tracker_with_pending_jobs_with_retry() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageContentTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        for n in 0..=2 {
            let job = JobStoredData {
                id: Some(
                    Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?.into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: (if n == 2 {
                    SchedulerJobMetadata {
                        job_type: SchedulerJob::WebPageTrackersTrigger {
                            kind: WebPageTrackerKind::WebPageContent,
                        },
                        retry: Some(SchedulerJobRetryState {
                            attempts: 1,
                            next_at: OffsetDateTime::now_utc().add(Duration::from_secs(3600)),
                        }),
                    }
                } else {
                    SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersTrigger {
                        kind: WebPageTrackerKind::WebPageContent,
                    })
                })
                .try_into()?,
                ran: true,
                stopped: n != 1,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
            };

            db.upsert_scheduler_job(&job).await?;
        }

        for n in 0..=2 {
            db.web_scraping(user.id)
                .insert_web_page_tracker(
                    &MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
                        Uuid::parse_str(&format!("77e55044-10b1-426f-9247-bb680e5fe0c{}", n))?,
                        format!("name_{}", n),
                        "https://secutils.dev",
                        3,
                    )?
                    .with_schedule("0 0 * * * *")
                    .with_job_id(Uuid::parse_str(&format!(
                        "67e55044-10b1-426f-9247-bb680e5fe0c{}",
                        n
                    ))?)
                    .build(),
                )
                .await?;
        }

        let mut pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageContentTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(pending_trackers.len(), 1);

        let tracker = pending_trackers.remove(0)?;
        assert_eq!(tracker.id, uuid!("77e55044-10b1-426f-9247-bb680e5fe0c0"));
        assert_eq!(
            tracker.job_id,
            Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c0"))
        );

        db.update_scheduler_job_meta(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c2"),
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                },
                retry: Some(SchedulerJobRetryState {
                    attempts: 1,
                    next_at: OffsetDateTime::now_utc().sub(Duration::from_secs(3600)),
                }),
            },
        )
        .await?;

        let mut pending_trackers = db
            .web_scraping_system()
            .get_pending_web_page_trackers::<WebPageContentTrackerTag>(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(pending_trackers.len(), 2);

        let tracker = pending_trackers.remove(0)?;
        assert_eq!(tracker.id, uuid!("77e55044-10b1-426f-9247-bb680e5fe0c0"));
        assert_eq!(
            tracker.job_id,
            Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c0"))
        );

        let tracker = pending_trackers.remove(0)?;
        assert_eq!(tracker.id, uuid!("77e55044-10b1-426f-9247-bb680e5fe0c2"));
        assert_eq!(
            tracker.job_id,
            Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c2"))
        );

        Ok(())
    }
}
