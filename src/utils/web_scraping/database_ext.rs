mod raw_api_tracker;
mod raw_page_tracker;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::UserId,
    utils::web_scraping::{ApiTracker, PageTracker},
};
use anyhow::{anyhow, bail};
use raw_api_tracker::RawApiTracker;
use raw_page_tracker::RawPageTracker;
use sqlx::{Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use uuid::Uuid;

/// A database extension for the web scraping utility-related operations.
pub struct WebScrapingDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
    user_id: UserId,
}

impl<'pool> WebScrapingDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>, user_id: UserId) -> Self {
        Self { pool, user_id }
    }

    /// Retrieves all page trackers.
    pub async fn get_page_trackers(&self) -> anyhow::Result<Vec<PageTracker>> {
        let raw_trackers = query_as!(
            RawPageTracker,
            r#"
SELECT id, name, retrack_id, user_id, secrets, created_at, updated_at
FROM user_data_web_scraping_page_trackers
WHERE user_id = $1
ORDER BY updated_at
                "#,
            *self.user_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut trackers = vec![];
        for raw_tracker in raw_trackers {
            trackers.push(PageTracker::try_from(raw_tracker)?);
        }

        Ok(trackers)
    }

    /// Retrieves page tracker for the specified user.
    pub async fn get_page_tracker(&self, id: Uuid) -> anyhow::Result<Option<PageTracker>> {
        query_as!(
            RawPageTracker,
            r#"
    SELECT id, name, user_id, retrack_id, secrets, created_at, updated_at
    FROM user_data_web_scraping_page_trackers
    WHERE user_id = $1 AND id = $2
                    "#,
            *self.user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(PageTracker::try_from)
        .transpose()
    }

    /// Inserts page tracker.
    pub async fn insert_page_tracker(&self, tracker: &PageTracker) -> anyhow::Result<()> {
        let raw_tracker = RawPageTracker::try_from(tracker)?;
        let result = query!(
            r#"
    INSERT INTO user_data_web_scraping_page_trackers (user_id, id, name, retrack_id, secrets, created_at, updated_at)
    VALUES ( $1, $2, $3, $4, $5, $6, $7 )
            "#,
            *self.user_id,
            raw_tracker.id,
            raw_tracker.name,
            raw_tracker.retrack_id,
            raw_tracker.secrets.as_slice(),
            raw_tracker.created_at,
            raw_tracker.updated_at
        )
            .execute(self.pool)
            .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::client_with_root_cause(
                    anyhow!(err)
                        .context(format!("Page tracker ('{}') already exists.", tracker.name)),
                )
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create page tracker ('{}') due to unknown reason.",
                    tracker.name
                )))
            });
        }

        Ok(())
    }

    /// Updates page tracker.
    pub async fn update_page_tracker(&self, tracker: &PageTracker) -> anyhow::Result<()> {
        let raw_tracker = RawPageTracker::try_from(tracker)?;
        let result = query!(
            r#"
UPDATE user_data_web_scraping_page_trackers
SET name = $3, retrack_id = $4, secrets = $5, updated_at = $6
WHERE user_id = $1 AND id = $2
        "#,
            *self.user_id,
            raw_tracker.id,
            raw_tracker.name,
            raw_tracker.retrack_id,
            raw_tracker.secrets.as_slice(),
            raw_tracker.updated_at
        )
        .execute(self.pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "A page tracker ('{}') doesn't exist.",
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
                    SecutilsError::client_with_root_cause(
                        anyhow!(err)
                            .context(format!("Page tracker ('{}') already exists.", tracker.name)),
                    )
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update page tracker ('{}') due to unknown reason.",
                        tracker.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes page tracker for the specified user with the specified ID.
    pub async fn remove_page_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        query!(
            r#"
    DELETE FROM user_data_web_scraping_page_trackers
    WHERE user_id = $1 AND id = $2
                    "#,
            *self.user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all API trackers.
    pub async fn get_api_trackers(&self) -> anyhow::Result<Vec<ApiTracker>> {
        let raw_trackers: Vec<RawApiTracker> = sqlx::query_as(
            r#"
SELECT id, name, user_id, retrack_id, secrets, created_at, updated_at
FROM user_data_web_scraping_api_trackers
WHERE user_id = $1
ORDER BY updated_at
            "#,
        )
        .bind(*self.user_id)
        .fetch_all(self.pool)
        .await?;

        let mut trackers = vec![];
        for raw_tracker in raw_trackers {
            trackers.push(ApiTracker::try_from(raw_tracker)?);
        }

        Ok(trackers)
    }

    /// Retrieves API tracker for the specified user.
    pub async fn get_api_tracker(&self, id: Uuid) -> anyhow::Result<Option<ApiTracker>> {
        let raw_tracker: Option<RawApiTracker> = sqlx::query_as(
            r#"
SELECT id, name, user_id, retrack_id, secrets, created_at, updated_at
FROM user_data_web_scraping_api_trackers
WHERE user_id = $1 AND id = $2
            "#,
        )
        .bind(*self.user_id)
        .bind(id)
        .fetch_optional(self.pool)
        .await?;

        raw_tracker.map(ApiTracker::try_from).transpose()
    }

    /// Inserts API tracker.
    pub async fn insert_api_tracker(&self, tracker: &ApiTracker) -> anyhow::Result<()> {
        let raw_tracker = RawApiTracker::try_from(tracker)?;
        let result = sqlx::query(
            r#"
INSERT INTO user_data_web_scraping_api_trackers (user_id, id, name, retrack_id, secrets, created_at, updated_at)
VALUES ( $1, $2, $3, $4, $5, $6, $7 )
            "#,
        )
        .bind(*self.user_id)
        .bind(raw_tracker.id)
        .bind(&raw_tracker.name)
        .bind(raw_tracker.retrack_id)
        .bind(raw_tracker.secrets.as_slice())
        .bind(raw_tracker.created_at)
        .bind(raw_tracker.updated_at)
        .execute(self.pool)
        .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::client_with_root_cause(
                    anyhow!(err)
                        .context(format!("API tracker ('{}') already exists.", tracker.name)),
                )
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create API tracker ('{}') due to unknown reason.",
                    tracker.name
                )))
            });
        }

        Ok(())
    }

    /// Updates API tracker.
    pub async fn update_api_tracker(&self, tracker: &ApiTracker) -> anyhow::Result<()> {
        let raw_tracker = RawApiTracker::try_from(tracker)?;
        let result = sqlx::query(
            r#"
UPDATE user_data_web_scraping_api_trackers
SET name = $3, retrack_id = $4, secrets = $5, updated_at = $6
WHERE user_id = $1 AND id = $2
            "#,
        )
        .bind(*self.user_id)
        .bind(raw_tracker.id)
        .bind(&raw_tracker.name)
        .bind(raw_tracker.retrack_id)
        .bind(raw_tracker.secrets.as_slice())
        .bind(raw_tracker.updated_at)
        .execute(self.pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    bail!(SecutilsError::client(format!(
                        "An API tracker ('{}') doesn't exist.",
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
                    SecutilsError::client_with_root_cause(
                        anyhow!(err)
                            .context(format!("API tracker ('{}') already exists.", tracker.name)),
                    )
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update API tracker ('{}') due to unknown reason.",
                        tracker.name
                    )))
                });
            }
        }

        Ok(())
    }

    /// Removes API tracker for the specified user with the specified ID.
    pub async fn remove_api_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            r#"
DELETE FROM user_data_web_scraping_api_trackers
WHERE user_id = $1 AND id = $2
            "#,
        )
        .bind(*self.user_id)
        .bind(id)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

impl Database {
    /// Returns a database extension for the web scraping utility-related operations performed on
    /// behalf of the specified user.
    pub fn web_scraping(&self, user_id: UserId) -> WebScrapingDatabaseExt<'_> {
        WebScrapingDatabaseExt::new(&self.pool, user_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        error::Error as SecutilsError,
        retrack::RetrackTracker,
        tests::{mock_user, to_database_error},
        utils::web_scraping::{
            ApiTracker, PageTracker,
            tests::{MockApiTrackerBuilder, MockPageTrackerBuilder},
        },
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_add_and_retrieve_page_trackers(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut trackers: Vec<PageTracker> = vec![
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            )?
            .build(),
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_page_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_page_tracker(trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker = web_scraping
            .get_page_tracker(trackers[0].id)
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        assert!(
            web_scraping
                .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000005"))
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_page_trackers_on_insert(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_page_tracker(&tracker).await?;

        let insert_error = web_scraping
            .insert_page_tracker(
                &MockPageTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_debug_snapshot!(
            insert_error.root_cause.to_string(),
            @r###""Page tracker ('some-name') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(insert_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_web_scraping_page_trackers_name_user_id_key\"""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_page_tracker(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        web_scraping
            .insert_page_tracker(
                &MockPageTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
                )?
                .build(),
            )
            .await?;
        web_scraping
            .update_page_tracker(
                &MockPageTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000011")),
                )?
                .build(),
            )
            .await?;

        let tracker = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            tracker,
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000011"))
            )?
            .build()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_page_trackers_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        let tracker_a = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();
        web_scraping.insert_page_tracker(&tracker_a).await?;

        let tracker_b = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "some-name-2",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
        )?
        .build();
        web_scraping.insert_page_tracker(&tracker_b).await?;

        let update_error = web_scraping
            .update_page_tracker(
                &MockPageTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""Page tracker ('some-name') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(update_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_web_scraping_page_trackers_name_user_id_key\"""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_non_existent_page_trackers_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let update_error = db
            .web_scraping(user.id)
            .update_page_tracker(
                &MockPageTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""A page tracker ('some-name-2') doesn't exist.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_page_trackers(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut trackers = vec![
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            )?
            .build(),
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_page_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker_2 = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker_2, trackers[0].clone());

        web_scraping
            .remove_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let tracker = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        web_scraping
            .remove_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let tracker = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(tracker.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_all_page_trackers(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            )?
            .build(),
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_page_tracker(tracker).await?;
        }

        assert_eq!(web_scraping.get_page_trackers().await?, trackers);

        Ok(())
    }

    #[sqlx::test]
    async fn can_add_and_retrieve_api_trackers(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut trackers: Vec<ApiTracker> = vec![
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            )?
            .build(),
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_api_tracker(tracker).await?;
        }

        let tracker = web_scraping.get_api_tracker(trackers[0].id).await?.unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker = web_scraping.get_api_tracker(trackers[0].id).await?.unwrap();
        assert_eq!(tracker, trackers.remove(0));

        assert!(
            web_scraping
                .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000005"))
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_api_trackers_on_insert(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_api_tracker(&tracker).await?;

        let insert_error = web_scraping
            .insert_api_tracker(
                &MockApiTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_debug_snapshot!(
            insert_error.root_cause.to_string(),
            @r###""API tracker ('some-name') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(insert_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_web_scraping_api_trackers_name_user_id_key\"""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_api_tracker(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        web_scraping
            .insert_api_tracker(
                &MockApiTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
                )?
                .build(),
            )
            .await?;
        web_scraping
            .update_api_tracker(
                &MockApiTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000011")),
                )?
                .build(),
            )
            .await?;

        let tracker = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            tracker,
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000011"))
            )?
            .build()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_duplicated_api_trackers_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let web_scraping = db.web_scraping(user.id);
        let tracker_a = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();
        web_scraping.insert_api_tracker(&tracker_a).await?;

        let tracker_b = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "some-name-2",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
        )?
        .build();
        web_scraping.insert_api_tracker(&tracker_b).await?;

        let update_error = web_scraping
            .update_api_tracker(
                &MockApiTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""API tracker ('some-name') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(update_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_web_scraping_api_trackers_name_user_id_key\"""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_non_existent_api_trackers_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let update_error = db
            .web_scraping(user.id)
            .update_api_tracker(
                &MockApiTrackerBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""An API tracker ('some-name-2') doesn't exist.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_api_trackers(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let mut trackers = vec![
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            )?
            .build(),
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_api_tracker(tracker).await?;
        }

        let tracker = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        let tracker_2 = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker_2, trackers[0].clone());

        web_scraping
            .remove_api_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let tracker = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(tracker, trackers.remove(0));

        web_scraping
            .remove_api_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let tracker = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(tracker.is_none());

        let tracker = web_scraping
            .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(tracker.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_all_api_trackers(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let trackers = vec![
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
            )?
            .build(),
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000020")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_api_tracker(tracker).await?;
        }

        assert_eq!(web_scraping.get_api_trackers().await?, trackers);

        Ok(())
    }
}
