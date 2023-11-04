mod raw_resources_revision;
mod raw_resources_tracker;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::UserId,
    utils::{
        web_scraping::database_ext::raw_resources_revision::RawResourcesRevision,
        WebPageResourcesRevision, WebPageResourcesTracker,
    },
};
use anyhow::{anyhow, bail};
use raw_resources_tracker::RawResourcesTracker;
use sqlx::{error::ErrorKind as SqlxErrorKind, query, query_as, Pool, Sqlite};
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

    /// Retrieves all resources trackers for the specified user.
    pub async fn get_resources_trackers(&self) -> anyhow::Result<Vec<WebPageResourcesTracker>> {
        let raw_trackers = query_as!(
            RawResourcesTracker,
            r#"
SELECT id, name, url, schedule, job_id, user_id, settings, created_at
FROM user_data_web_scraping_resources
WHERE user_id = ?1
ORDER BY created_at
                "#,
            *self.user_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut trackers = vec![];
        for raw_tracker in raw_trackers {
            trackers.push(WebPageResourcesTracker::try_from(raw_tracker)?);
        }

        Ok(trackers)
    }

    /// Retrieves resources tracker for the specified user with the specified ID.
    pub async fn get_resources_tracker(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<WebPageResourcesTracker>> {
        let id = id.as_ref();
        query_as!(
            RawResourcesTracker,
            r#"
    SELECT id, name, url, schedule, user_id, job_id, settings, created_at
    FROM user_data_web_scraping_resources
    WHERE user_id = ?1 AND id = ?2
                    "#,
            *self.user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(WebPageResourcesTracker::try_from)
        .transpose()
    }

    /// Inserts resources tracker.
    pub async fn insert_resources_tracker(
        &self,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<()> {
        let raw_tracker = RawResourcesTracker::try_from(tracker)?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_scraping_resources (user_id, id, name, url, schedule, job_id, settings, created_at)
    VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8 )
            "#,
            *self.user_id,
            raw_tracker.id,
            raw_tracker.name,
            raw_tracker.url,
            raw_tracker.schedule,
            raw_tracker.job_id,
            raw_tracker.settings,
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
                    "Resources tracker ('{}') already exists.",
                    tracker.name
                )))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create resources tracker ('{}') due to unknown reason.",
                    tracker.name
                )))
            });
        }

        Ok(())
    }

    /// Updates web page resources tracker.
    pub async fn update_resources_tracker(
        &self,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<()> {
        let raw_tracker = RawResourcesTracker::try_from(tracker)?;
        let result = query!(
            r#"
UPDATE user_data_web_scraping_resources
SET name = ?3, url = ?4, schedule = ?5, settings = ?6, job_id = ?7
WHERE user_id = ?1 AND id = ?2
        "#,
            *self.user_id,
            raw_tracker.id,
            raw_tracker.name,
            raw_tracker.url,
            raw_tracker.schedule,
            raw_tracker.settings,
            raw_tracker.job_id
        )
        .execute(self.pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "A resources tracker ('{}') doesn't exist.",
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
                        "Resources tracker ('{}') already exists.",
                        tracker.name
                    )))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update resources tracker ('{}') due to unknown reason.",
                        tracker.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes resources tracker for the specified user with the specified ID.
    pub async fn remove_resources_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        let id = id.as_ref();
        query!(
            r#"
    DELETE FROM user_data_web_scraping_resources
    WHERE user_id = ?1 AND id = ?2
                    "#,
            *self.user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all tracked revisions for the specified resources tracker.
    pub async fn get_resources_tracker_history(
        &self,
        tracker_id: Uuid,
    ) -> anyhow::Result<Vec<WebPageResourcesRevision>> {
        let raw_revisions = query_as!(
            RawResourcesRevision,
            r#"
SELECT id, tracker_id, value, created_at
FROM user_data_web_scraping_resources_history
WHERE user_id = ?1 AND tracker_id = ?2
ORDER BY created_at
                "#,
            *self.user_id,
            tracker_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut revisions = vec![];
        for raw_revision in raw_revisions {
            revisions.push(WebPageResourcesRevision::try_from(raw_revision)?);
        }

        Ok(revisions)
    }

    /// Removes resources tracker history.
    pub async fn clear_resources_tracker_history(&self, tracker_id: Uuid) -> anyhow::Result<()> {
        let id = tracker_id.as_ref();
        query!(
            r#"
    DELETE FROM user_data_web_scraping_resources_history
    WHERE user_id = ?1 AND tracker_id = ?2
                    "#,
            *self.user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    // Inserts resources tracker revision.
    pub async fn insert_resources_tracker_history_revision(
        &self,
        revision: &WebPageResourcesRevision,
    ) -> anyhow::Result<()> {
        let raw_revision = RawResourcesRevision::try_from(revision)?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_scraping_resources_history (user_id, id, tracker_id, value, created_at)
    VALUES ( ?1, ?2, ?3, ?4, ?5 )
            "#,
            *self.user_id,
            raw_revision.id,
            raw_revision.tracker_id,
            raw_revision.value,
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
                    "Resources tracker revision ('{}') already exists.",
                    revision.id
                )))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create resources tracker revision ('{}') due to unknown reason.",
                    revision.id
                )))
            });
        }

        Ok(())
    }

    /// Removes resources tracker history.
    pub async fn remove_resources_tracker_history_revision(
        &self,
        tracker_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<()> {
        query!(
            r#"
    DELETE FROM user_data_web_scraping_resources_history
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

    /// Retrieves all resources trackers that need to be scheduled.
    pub async fn get_unscheduled_resources_trackers(
        &self,
    ) -> anyhow::Result<Vec<WebPageResourcesTracker>> {
        let raw_trackers = query_as!(
            RawResourcesTracker,
            r#"
SELECT id, name, url, schedule, user_id, job_id, settings, created_at
FROM user_data_web_scraping_resources
WHERE schedule IS NOT NULL AND job_id IS NULL
ORDER BY created_at
                "#
        )
        .fetch_all(self.pool)
        .await?;

        let mut trackers = vec![];
        for raw_tracker in raw_trackers {
            let tracker = WebPageResourcesTracker::try_from(raw_tracker)?;
            // Tracker without revisions shouldn't be scheduled.
            if tracker.settings.revisions > 0 {
                trackers.push(tracker);
            }
        }

        Ok(trackers)
    }

    /// Retrieves resources tracker by the specified job ID.
    pub async fn get_resources_tracker_by_job_id(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<WebPageResourcesTracker>> {
        query_as!(
            RawResourcesTracker,
            r#"
    SELECT id, name, url, schedule, user_id, job_id, settings, created_at
    FROM user_data_web_scraping_resources
    WHERE job_id = ?1
                    "#,
            job_id
        )
        .fetch_optional(self.pool)
        .await?
        .map(WebPageResourcesTracker::try_from)
        .transpose()
    }

    /// Inserts resources tracker.
    pub async fn update_resources_tracker_job(
        &self,
        id: Uuid,
        job_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        let result = query!(
            r#"
    UPDATE user_data_web_scraping_resources
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
                "A resource tracker ('{id}') doesn't exist.",
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
        tests::{mock_db, mock_user, MockWebPageResourcesTrackerBuilder},
        utils::{
            WebPageResource, WebPageResourceContent, WebPageResourceContentData,
            WebPageResourcesRevision, WebPageResourcesTracker,
        },
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::{uuid, Uuid};

    fn create_resources_revision(
        id: Uuid,
        tracker_id: Uuid,
        time_shift: i64,
    ) -> anyhow::Result<WebPageResourcesRevision> {
        Ok(WebPageResourcesRevision {
            id,
            tracker_id,
            created_at: OffsetDateTime::from_unix_timestamp(946720800 + time_shift)?,
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
        })
    }

    #[actix_rt::test]
    async fn can_add_and_retrieve_resources_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_resources_tracker(trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker = web_scraping
            .get_resources_tracker(trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        assert!(web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000003"))
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_handles_duplicated_resources_trackers_on_insert() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_resources_tracker(&tracker).await?;

        let insert_error = web_scraping
            .insert_resources_tracker(
                &MockWebPageResourcesTrackerBuilder::create(
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
            context: "Resources tracker (\'some-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_scraping_resources.name, user_data_web_scraping_resources.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_resources_tracker() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        web_scraping
            .insert_resources_tracker(
                &MockWebPageResourcesTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    "https://secutils.dev",
                    3,
                )?
                .build(),
            )
            .await?;

        web_scraping
            .update_resources_tracker(
                &MockWebPageResourcesTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    "https://secutils.dev",
                    5,
                )?
                .build(),
            )
            .await?;

        let tracker = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            tracker,
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name-2",
                "https://secutils.dev",
                5,
            )?
            .build()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_handles_duplicated_resources_trackers_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        let tracker_a = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .build();
        web_scraping.insert_resources_tracker(&tracker_a).await?;

        let tracker_b = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "some-name-2",
            "https://secutils.dev",
            3,
        )?
        .build();
        web_scraping.insert_resources_tracker(&tracker_b).await?;

        let update_error = web_scraping
            .update_resources_tracker(
                &MockWebPageResourcesTrackerBuilder::create(
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
            context: "Resources tracker (\'some-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_web_scraping_resources.name, user_data_web_scraping_resources.user_id",
                },
            ),
        }
        "###
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_handles_non_existent_resources_trackers_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let update_error = db
            .web_scraping(user.id)
            .update_resources_tracker(
                &MockWebPageResourcesTrackerBuilder::create(
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
            @r###""A resources tracker ('some-name-2') doesn't exist.""###
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_resources_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker_2 = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker_2, trackers[0].clone());

        web_scraping
            .remove_resources_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let tracker = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        web_scraping
            .remove_resources_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let tracker = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(tracker.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_all_resources_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
        }

        assert_eq!(web_scraping.get_resources_trackers().await?, trackers);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_add_and_retrieve_resources_revisions() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
        }

        // No history yet.
        for tracker in trackers.iter() {
            assert!(web_scraping
                .get_resources_tracker_history(tracker.id)
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
                .insert_resources_tracker_history_revision(revision)
                .await?;
        }

        let history = web_scraping
            .get_resources_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions.remove(0), revisions.remove(0)]);

        let history = web_scraping
            .get_resources_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions.remove(0)]);

        assert!(web_scraping
            .get_resources_tracker_history(uuid!("00000000-0000-0000-0000-000000000004"))
            .await?
            .is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_resources_revisions() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
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
                .insert_resources_tracker_history_revision(revision)
                .await?;
        }

        let history = web_scraping
            .get_resources_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions[0].clone(), revisions[1].clone()]);

        let history = web_scraping
            .get_resources_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions[2].clone()]);

        // Remove one revision.
        web_scraping
            .remove_resources_tracker_history_revision(trackers[0].id, revisions[0].id)
            .await?;

        let history = web_scraping
            .get_resources_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions[1].clone()]);

        let history = web_scraping
            .get_resources_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions[2].clone()]);

        // Remove the rest of revisions.
        web_scraping
            .remove_resources_tracker_history_revision(trackers[0].id, revisions[1].id)
            .await?;
        web_scraping
            .remove_resources_tracker_history_revision(trackers[1].id, revisions[2].id)
            .await?;

        assert!(web_scraping
            .get_resources_tracker_history(trackers[0].id)
            .await?
            .is_empty());
        assert!(web_scraping
            .get_resources_tracker_history(trackers[1].id)
            .await?
            .is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_clear_all_resources_revisions_at_once() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
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
                .insert_resources_tracker_history_revision(revision)
                .await?;
        }

        let history = web_scraping
            .get_resources_tracker_history(trackers[0].id)
            .await?;
        assert_eq!(history, vec![revisions[0].clone(), revisions[1].clone()]);

        let history = web_scraping
            .get_resources_tracker_history(trackers[1].id)
            .await?;
        assert_eq!(history, vec![revisions[2].clone()]);

        // Clear all revisions.
        web_scraping
            .clear_resources_tracker_history(trackers[0].id)
            .await?;
        web_scraping
            .clear_resources_tracker_history(trackers[1].id)
            .await?;

        assert!(web_scraping
            .get_resources_tracker_history(trackers[0].id)
            .await?
            .is_empty());
        assert!(web_scraping
            .get_resources_tracker_history(trackers[1].id)
            .await?
            .is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_all_unscheduled_resources_trackers() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name-3",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000004"),
                "some-name-4",
                "https://secutils.dev",
                3,
            )?
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000005"),
                "some-name-5",
                "https://secutils.dev",
                0,
            )?
            .with_schedule("* * * * *")
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_resources_tracker(tracker).await?;
        }

        assert_eq!(web_scraping.get_resources_trackers().await?, trackers);

        let web_scraping_system = db.web_scraping_system();
        assert_eq!(
            web_scraping_system
                .get_unscheduled_resources_trackers()
                .await?,
            vec![
                trackers[0].clone(),
                trackers[1].clone(),
                trackers[2].clone()
            ]
        );

        web_scraping_system
            .update_resources_tracker_job(
                trackers[1].id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping.get_resources_trackers().await?,
            vec![
                trackers[0].clone(),
                WebPageResourcesTracker {
                    job_id: Some(uuid!("00000000-0000-0000-0000-000000000001")),
                    ..trackers[1].clone()
                },
                trackers[2].clone(),
                trackers[3].clone(),
                trackers[4].clone(),
            ]
        );
        assert_eq!(
            web_scraping_system
                .get_unscheduled_resources_trackers()
                .await?,
            vec![trackers[0].clone(), trackers[2].clone()]
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_resources_tracker_by_job_id() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .with_job_id(uuid!("00000000-0000-0000-0000-000000000011"))
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "https://secutils.dev",
                3,
            )?
            .with_schedule("* * * * *")
            .with_job_id(uuid!("00000000-0000-0000-0000-000000000022"))
            .build(),
            MockWebPageResourcesTrackerBuilder::create(
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
            web_scraping.insert_resources_tracker(tracker).await?;
        }

        let web_scraping_system = db.web_scraping_system();
        assert_eq!(
            web_scraping_system
                .get_resources_tracker_by_job_id(uuid!("00000000-0000-0000-0000-000000000011"))
                .await?,
            Some(trackers[0].clone())
        );
        assert_eq!(
            web_scraping_system
                .get_resources_tracker_by_job_id(uuid!("00000000-0000-0000-0000-000000000022"))
                .await?,
            Some(trackers[1].clone())
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_update_resources_trackers_job_id() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "https://secutils.dev",
            3,
        )?
        .with_schedule("* * * * *")
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_resources_tracker(&tracker).await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            None
        );

        let web_scraping_system = db.web_scraping_system();
        web_scraping_system
            .update_resources_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000011")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000011"))
        );

        web_scraping_system
            .update_resources_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000022")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000022"))
        );

        Ok(())
    }

    #[tokio::test]
    async fn fails_to_update_resources_trackers_job_id_if_needed() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let tracker = MockWebPageResourcesTrackerBuilder::create(
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
                .update_resources_tracker_job(
                    tracker.id,
                    Some(uuid!("00000000-0000-0000-0000-000000000011")),
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!("A resource tracker ('{}') doesn't exist.", tracker.id)
        );

        Ok(())
    }
}
