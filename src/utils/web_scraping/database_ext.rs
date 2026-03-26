mod raw_api_tracker;
mod raw_page_tracker;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::{EntityTag, RawEntityTag, UserId, group_entity_tags},
    utils::web_scraping::{ApiTracker, PageTracker},
};
use anyhow::{anyhow, bail};
use raw_api_tracker::RawApiTracker;
use raw_page_tracker::RawPageTracker;
use sqlx::{Acquire, Pool, Postgres, error::ErrorKind as SqlxErrorKind, query, query_as};
use std::collections::HashMap;
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

    /// Retrieves page trackers matching the given IDs.
    pub async fn bulk_get_page_trackers(&self, ids: &[Uuid]) -> anyhow::Result<Vec<PageTracker>> {
        let raw_trackers = query_as!(
            RawPageTracker,
            r#"
SELECT id, name, retrack_id, user_id, secrets, created_at, updated_at
FROM user_data_web_scraping_page_trackers
WHERE user_id = $1 AND id = ANY($2)
ORDER BY updated_at
                "#,
            *self.user_id,
            ids
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

    /// Inserts page tracker (and associated tags). Returns resolved tags.
    pub async fn insert_page_tracker(
        &self,
        tracker: &PageTracker,
    ) -> anyhow::Result<Vec<EntityTag>> {
        let raw_tracker = RawPageTracker::try_from(tracker)?;
        let mut tx = self.pool.begin().await?;
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
            .execute(&mut *tx)
            .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::conflict(format!(
                    "Page tracker ('{}') already exists.",
                    tracker.name
                ))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create page tracker ('{}') due to unknown reason.",
                    tracker.name
                )))
            });
        }

        let tags = if tracker.tags.is_empty() {
            vec![]
        } else {
            Self::set_page_tracker_tags(
                &mut *tx,
                tracker.id,
                &tracker.tags.iter().map(|t| t.id).collect::<Vec<_>>(),
            )
            .await?
        };

        tx.commit().await?;
        Ok(tags)
    }

    /// Updates page tracker (and associated tags). Returns resolved tags.
    pub async fn update_page_tracker(
        &self,
        tracker: &PageTracker,
        tag_ids: Option<Vec<Uuid>>,
    ) -> anyhow::Result<Option<Vec<EntityTag>>> {
        let raw_tracker = RawPageTracker::try_from(tracker)?;
        let mut tx = self.pool.begin().await?;
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
        .execute(&mut *tx)
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
                    SecutilsError::conflict(format!(
                        "Page tracker ('{}') already exists.",
                        tracker.name
                    ))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update page tracker ('{}') due to unknown reason.",
                        tracker.name
                    )))
                });
            }
        }

        let updated_tags = if let Some(ref tag_ids) = tag_ids {
            Some(Self::set_page_tracker_tags(&mut *tx, tracker.id, tag_ids).await?)
        } else {
            None
        };

        tx.commit().await?;
        Ok(updated_tags)
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

    /// Retrieves API trackers matching the given IDs.
    pub async fn bulk_get_api_trackers(&self, ids: &[Uuid]) -> anyhow::Result<Vec<ApiTracker>> {
        let raw_trackers: Vec<RawApiTracker> = sqlx::query_as(
            r#"
SELECT id, name, user_id, retrack_id, secrets, created_at, updated_at
FROM user_data_web_scraping_api_trackers
WHERE user_id = $1 AND id = ANY($2)
ORDER BY updated_at
            "#,
        )
        .bind(*self.user_id)
        .bind(ids)
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

    /// Inserts API tracker (and associated tags). Returns resolved tags.
    pub async fn insert_api_tracker(&self, tracker: &ApiTracker) -> anyhow::Result<Vec<EntityTag>> {
        let raw_tracker = RawApiTracker::try_from(tracker)?;
        let mut tx = self.pool.begin().await?;
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
        .execute(&mut *tx)
        .await;

        if let Err(err) = result {
            let is_conflict_error = err
                .as_database_error()
                .map(|db_error| matches!(db_error.kind(), SqlxErrorKind::UniqueViolation))
                .unwrap_or_default();
            bail!(if is_conflict_error {
                SecutilsError::conflict(format!("API tracker ('{}') already exists.", tracker.name))
            } else {
                SecutilsError::from(anyhow!(err).context(format!(
                    "Couldn't create API tracker ('{}') due to unknown reason.",
                    tracker.name
                )))
            });
        }

        let tags = if tracker.tags.is_empty() {
            vec![]
        } else {
            Self::set_api_tracker_tags(
                &mut *tx,
                tracker.id,
                &tracker.tags.iter().map(|t| t.id).collect::<Vec<_>>(),
            )
            .await?
        };

        tx.commit().await?;
        Ok(tags)
    }

    /// Updates API tracker (and associated tags). Returns resolved tags.
    pub async fn update_api_tracker(
        &self,
        tracker: &ApiTracker,
        tag_ids: Option<Vec<Uuid>>,
    ) -> anyhow::Result<Option<Vec<EntityTag>>> {
        let raw_tracker = RawApiTracker::try_from(tracker)?;
        let mut tx = self.pool.begin().await?;
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
        .execute(&mut *tx)
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
                    SecutilsError::conflict(format!(
                        "API tracker ('{}') already exists.",
                        tracker.name
                    ))
                } else {
                    SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update API tracker ('{}') due to unknown reason.",
                        tracker.name
                    )))
                });
            }
        }

        let updated_tags = if let Some(ref tag_ids) = tag_ids {
            Some(Self::set_api_tracker_tags(&mut *tx, tracker.id, tag_ids).await?)
        } else {
            None
        };

        tx.commit().await?;
        Ok(updated_tags)
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

    /// Fetches tags for a batch of page trackers.
    pub async fn get_page_tracker_tags(
        &self,
        entity_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, Vec<EntityTag>>> {
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = query_as!(
            RawEntityTag,
            r#"
SELECT jt.tracker_id AS entity_id, t.id, t.name, t.color
FROM user_data_web_scraping_page_trackers_tags jt
JOIN user_tags t ON jt.tag_id = t.id
WHERE jt.tracker_id = ANY($1)
ORDER BY t.name ASC
            "#,
            entity_ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(group_entity_tags(rows))
    }

    async fn set_page_tracker_tags<'a>(
        executor: impl Acquire<'a, Database = Postgres>,
        entity_id: Uuid,
        tag_ids: &[Uuid],
    ) -> anyhow::Result<Vec<EntityTag>> {
        let mut conn = executor.acquire().await?;
        query!(
            "DELETE FROM user_data_web_scraping_page_trackers_tags WHERE tracker_id = $1",
            entity_id
        )
        .execute(&mut *conn)
        .await?;

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        Ok(query_as!(
            EntityTag,
            r#"
WITH inserted AS (
    INSERT INTO user_data_web_scraping_page_trackers_tags (tracker_id, tag_id)
    SELECT $1, unnest($2::uuid[])
    RETURNING tracker_id, tag_id
)
SELECT t.id, t.name, t.color
FROM inserted i
JOIN user_tags t ON i.tag_id = t.id
ORDER BY t.name ASC
            "#,
            entity_id,
            tag_ids
        )
        .fetch_all(&mut *conn)
        .await?)
    }

    /// Fetches tags for a batch of API trackers.
    pub async fn get_api_tracker_tags(
        &self,
        entity_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, Vec<EntityTag>>> {
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = query_as!(
            RawEntityTag,
            r#"
SELECT jt.tracker_id AS entity_id, t.id, t.name, t.color
FROM user_data_web_scraping_api_trackers_tags jt
JOIN user_tags t ON jt.tag_id = t.id
WHERE jt.tracker_id = ANY($1)
ORDER BY t.name ASC
            "#,
            entity_ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(group_entity_tags(rows))
    }

    async fn set_api_tracker_tags<'a>(
        executor: impl Acquire<'a, Database = Postgres>,
        entity_id: Uuid,
        tag_ids: &[Uuid],
    ) -> anyhow::Result<Vec<EntityTag>> {
        let mut conn = executor.acquire().await?;
        query!(
            "DELETE FROM user_data_web_scraping_api_trackers_tags WHERE tracker_id = $1",
            entity_id
        )
        .execute(&mut *conn)
        .await?;

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        Ok(query_as!(
            EntityTag,
            r#"
WITH inserted AS (
    INSERT INTO user_data_web_scraping_api_trackers_tags (tracker_id, tag_id)
    SELECT $1, unnest($2::uuid[])
    RETURNING tracker_id, tag_id
)
SELECT t.id, t.name, t.color
FROM inserted i
JOIN user_tags t ON i.tag_id = t.id
ORDER BY t.name ASC
            "#,
            entity_id,
            tag_ids
        )
        .fetch_all(&mut *conn)
        .await?)
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
        tests::{mock_user, mock_user_with_id},
        users::EntityTag,
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
                None,
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
                None,
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""Page tracker ('some-name') already exists.""###
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
                None,
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
                None,
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
                None,
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error.root_cause.to_string(),
            @r###""API tracker ('some-name') already exists.""###
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
                None,
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

    #[sqlx::test]
    async fn can_bulk_get_page_trackers_empty(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let trackers = db.web_scraping(user.id).bulk_get_page_trackers(&[]).await?;
        assert!(trackers.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_page_trackers_returns_matching(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let trackers = [
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
            MockPageTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name-3",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000030")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_page_tracker(tracker).await?;
        }

        let result = web_scraping
            .bulk_get_page_trackers(&[
                uuid!("00000000-0000-0000-0000-000000000001"),
                uuid!("00000000-0000-0000-0000-000000000003"),
            ])
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], trackers[0]);
        assert_eq!(result[1], trackers[2]);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_page_trackers_ignores_non_existent(pool: PgPool) -> anyhow::Result<()> {
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

        let result = web_scraping
            .bulk_get_page_trackers(&[
                uuid!("00000000-0000-0000-0000-000000000001"),
                uuid!("00000000-0000-0000-0000-000000000099"),
            ])
            .await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tracker);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_page_trackers_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();

        db.web_scraping(user_a.id)
            .insert_page_tracker(&tracker)
            .await?;

        let result = db
            .web_scraping(user_b.id)
            .bulk_get_page_trackers(&[uuid!("00000000-0000-0000-0000-000000000001")])
            .await?;
        assert!(result.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_api_trackers_empty(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let trackers = db.web_scraping(user.id).bulk_get_api_trackers(&[]).await?;
        assert!(trackers.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_bulk_get_api_trackers_returns_matching(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let trackers = [
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
            MockApiTrackerBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name-3",
                RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000030")),
            )?
            .build(),
        ];

        let web_scraping = db.web_scraping(user.id);
        for tracker in trackers.iter() {
            web_scraping.insert_api_tracker(tracker).await?;
        }

        let result = web_scraping
            .bulk_get_api_trackers(&[
                uuid!("00000000-0000-0000-0000-000000000001"),
                uuid!("00000000-0000-0000-0000-000000000003"),
            ])
            .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], trackers[0]);
        assert_eq!(result[1], trackers[2]);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_api_trackers_ignores_non_existent(pool: PgPool) -> anyhow::Result<()> {
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

        let result = web_scraping
            .bulk_get_api_trackers(&[
                uuid!("00000000-0000-0000-0000-000000000001"),
                uuid!("00000000-0000-0000-0000-000000000099"),
            ])
            .await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tracker);

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_api_trackers_isolated_per_user(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();

        db.web_scraping(user_a.id)
            .insert_api_tracker(&tracker)
            .await?;

        let result = db
            .web_scraping(user_b.id)
            .bulk_get_api_trackers(&[uuid!("00000000-0000-0000-0000-000000000001")])
            .await?;
        assert!(result.is_empty());

        Ok(())
    }

    // ── Page tracker tag tests ──────────────────────────────────────────

    #[sqlx::test]
    async fn can_set_and_get_page_tracker_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let tags = web_scraping.insert_page_tracker(&tracker).await?;
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags,
            vec![
                EntityTag {
                    id: tag_a.id,
                    name: "alpha".to_string(),
                    color: "primary".to_string()
                },
                EntityTag {
                    id: tag_b.id,
                    name: "beta".to_string(),
                    color: "danger".to_string()
                },
            ]
        );

        let tags_map = web_scraping.get_page_tracker_tags(&[tracker.id]).await?;
        assert_eq!(tags_map[&tracker.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn update_page_tracker_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_page_tracker(&tracker).await?;

        let tags = web_scraping
            .update_page_tracker(&tracker, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let tags_map = web_scraping.get_page_tracker_tags(&[tracker.id]).await?;
        let tag_names: Vec<&str> = tags_map[&tracker.id]
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(tag_names, vec!["beta", "gamma"]);

        Ok(())
    }

    #[sqlx::test]
    async fn update_page_tracker_clears_all_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_page_tracker(&tracker).await?;

        let tags = web_scraping
            .update_page_tracker(&tracker, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = web_scraping.get_page_tracker_tags(&[tracker.id]).await?;
        assert!(!tags_map.contains_key(&tracker.id) || tags_map[&tracker.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_with_nonexistent_tag_ids_fails(
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
        .with_tag_ids(&[uuid!("00000000-0000-0000-0000-000000000099")])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let result = web_scraping.insert_page_tracker(&tracker).await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_returns_tags_ordered_by_name(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_z = db.insert_user_tag(user.id, "zebra", "primary").await?;
        let tag_a = db.insert_user_tag(user.id, "alpha", "danger").await?;
        let tag_m = db.insert_user_tag(user.id, "middle", "success").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_z.id, tag_a.id, tag_m.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let tags = web_scraping.insert_page_tracker(&tracker).await?;
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_with_tags_is_atomic(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let tags = web_scraping.insert_page_tracker(&tracker).await?;

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[1].name, "beta");

        let fetched = web_scraping.get_page_tracker(tracker.id).await?;
        assert!(fetched.is_some());

        let tags_map = web_scraping.get_page_tracker_tags(&[tracker.id]).await?;
        assert_eq!(tags_map[&tracker.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_with_tags_empty_tags(pool: PgPool) -> anyhow::Result<()> {
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
        let tags = web_scraping.insert_page_tracker(&tracker).await?;

        assert!(tags.is_empty());

        let fetched = web_scraping.get_page_tracker(tracker.id).await?;
        assert!(fetched.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn update_page_tracker_with_tags_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_page_tracker(&tracker).await?;

        let updated = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "updated-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();
        let tags = web_scraping
            .update_page_tracker(&updated, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let fetched = web_scraping.get_page_tracker(tracker.id).await?.unwrap();
        assert_eq!(fetched.name, "updated-name");

        Ok(())
    }

    #[sqlx::test]
    async fn update_page_tracker_with_tags_clears_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_page_tracker(&tracker).await?;

        let tags = web_scraping
            .update_page_tracker(&tracker, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = web_scraping.get_page_tracker_tags(&[tracker.id]).await?;
        assert!(!tags_map.contains_key(&tracker.id) || tags_map[&tracker.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_with_tags_rolls_back_on_invalid_tags(
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
        .with_tag_ids(&[uuid!("00000000-0000-0000-0000-000000000099")])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let result = web_scraping.insert_page_tracker(&tracker).await;
        assert!(result.is_err());

        let fetched = web_scraping.get_page_tracker(tracker.id).await?;
        assert!(fetched.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_with_tags_handles_duplicate_tag_ids(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id, tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let result = web_scraping.insert_page_tracker(&tracker).await;

        match result {
            Ok(tags) => {
                assert_eq!(tags.len(), 1);
                assert_eq!(tags[0].name, "alpha");
            }
            Err(_) => {
                let fetched = web_scraping.get_page_tracker(tracker.id).await?;
                assert!(fetched.is_none());
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn update_page_tracker_with_tags_rolls_back_on_invalid_tags(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_page_tracker(&tracker).await?;

        let updated = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "updated-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();
        let result = web_scraping
            .update_page_tracker(
                &updated,
                Some(vec![uuid!("00000000-0000-0000-0000-000000000099")]),
            )
            .await;
        assert!(result.is_err());

        let fetched = web_scraping.get_page_tracker(tracker.id).await?.unwrap();
        assert_eq!(fetched.name, "some-name");

        let tags_map = web_scraping.get_page_tracker_tags(&[tracker.id]).await?;
        assert_eq!(tags_map[&tracker.id].len(), 1);
        assert_eq!(tags_map[&tracker.id][0].name, "alpha");

        Ok(())
    }

    #[sqlx::test]
    async fn insert_page_tracker_with_tags_isolated_between_users(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let tag_a = db.insert_user_tag(user_a.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user_b.id, "beta", "danger").await?;

        let tracker_a = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "tracker-a",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id])
        .build();
        let tracker_b = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "tracker-b",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000030")),
        )?
        .with_tag_ids(&[tag_b.id])
        .build();

        let tags_a = db
            .web_scraping(user_a.id)
            .insert_page_tracker(&tracker_a)
            .await?;
        let tags_b = db
            .web_scraping(user_b.id)
            .insert_page_tracker(&tracker_b)
            .await?;

        assert_eq!(tags_a.len(), 1);
        assert_eq!(tags_a[0].name, "alpha");
        assert_eq!(tags_b.len(), 1);
        assert_eq!(tags_b[0].name, "beta");

        let map_a = db
            .web_scraping(user_a.id)
            .get_page_tracker_tags(&[tracker_a.id])
            .await?;
        let map_b = db
            .web_scraping(user_b.id)
            .get_page_tracker_tags(&[tracker_b.id])
            .await?;
        assert_eq!(map_a[&tracker_a.id].len(), 1);
        assert_eq!(map_b[&tracker_b.id].len(), 1);
        assert_eq!(map_a[&tracker_a.id][0].name, "alpha");
        assert_eq!(map_b[&tracker_b.id][0].name, "beta");

        Ok(())
    }

    // ── API tracker tag tests ───────────────────────────────────────────

    #[sqlx::test]
    async fn can_set_and_get_api_tracker_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let tags = web_scraping.insert_api_tracker(&tracker).await?;
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags,
            vec![
                EntityTag {
                    id: tag_a.id,
                    name: "alpha".to_string(),
                    color: "primary".to_string()
                },
                EntityTag {
                    id: tag_b.id,
                    name: "beta".to_string(),
                    color: "danger".to_string()
                },
            ]
        );

        let tags_map = web_scraping.get_api_tracker_tags(&[tracker.id]).await?;
        assert_eq!(tags_map[&tracker.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_tracker_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_api_tracker(&tracker).await?;

        let tags = web_scraping
            .update_api_tracker(&tracker, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let tags_map = web_scraping.get_api_tracker_tags(&[tracker.id]).await?;
        let tag_names: Vec<&str> = tags_map[&tracker.id]
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(tag_names, vec!["beta", "gamma"]);

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_tracker_clears_all_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_api_tracker(&tracker).await?;

        let tags = web_scraping
            .update_api_tracker(&tracker, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = web_scraping.get_api_tracker_tags(&[tracker.id]).await?;
        assert!(!tags_map.contains_key(&tracker.id) || tags_map[&tracker.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_with_nonexistent_tag_ids_fails(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[uuid!("00000000-0000-0000-0000-000000000099")])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let result = web_scraping.insert_api_tracker(&tracker).await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_returns_tags_ordered_by_name(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_z = db.insert_user_tag(user.id, "zebra", "primary").await?;
        let tag_a = db.insert_user_tag(user.id, "alpha", "danger").await?;
        let tag_m = db.insert_user_tag(user.id, "middle", "success").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_z.id, tag_a.id, tag_m.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let tags = web_scraping.insert_api_tracker(&tracker).await?;
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_with_tags_is_atomic(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let tags = web_scraping.insert_api_tracker(&tracker).await?;

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[1].name, "beta");

        let fetched = web_scraping.get_api_tracker(tracker.id).await?;
        assert!(fetched.is_some());

        let tags_map = web_scraping.get_api_tracker_tags(&[tracker.id]).await?;
        assert_eq!(tags_map[&tracker.id], tags);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_with_tags_empty_tags(pool: PgPool) -> anyhow::Result<()> {
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
        let tags = web_scraping.insert_api_tracker(&tracker).await?;

        assert!(tags.is_empty());

        let fetched = web_scraping.get_api_tracker(tracker.id).await?;
        assert!(fetched.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_tracker_with_tags_replaces_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag_a = db.insert_user_tag(user.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user.id, "beta", "danger").await?;
        let tag_c = db.insert_user_tag(user.id, "gamma", "success").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id, tag_b.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_api_tracker(&tracker).await?;

        let updated = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "updated-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();
        let tags = web_scraping
            .update_api_tracker(&updated, Some(vec![tag_b.id, tag_c.id]))
            .await?
            .unwrap();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "beta");
        assert_eq!(tags[1].name, "gamma");

        let fetched = web_scraping.get_api_tracker(tracker.id).await?.unwrap();
        assert_eq!(fetched.name, "updated-name");

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_tracker_with_tags_clears_tags(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_api_tracker(&tracker).await?;

        let tags = web_scraping
            .update_api_tracker(&tracker, Some(vec![]))
            .await?
            .unwrap();
        assert!(tags.is_empty());

        let tags_map = web_scraping.get_api_tracker_tags(&[tracker.id]).await?;
        assert!(!tags_map.contains_key(&tracker.id) || tags_map[&tracker.id].is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_with_tags_rolls_back_on_invalid_tags(
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
        .with_tag_ids(&[uuid!("00000000-0000-0000-0000-000000000099")])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let result = web_scraping.insert_api_tracker(&tracker).await;
        assert!(result.is_err());

        let fetched = web_scraping.get_api_tracker(tracker.id).await?;
        assert!(fetched.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_with_tags_handles_duplicate_tag_ids(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id, tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        let result = web_scraping.insert_api_tracker(&tracker).await;

        match result {
            Ok(tags) => {
                assert_eq!(tags.len(), 1);
                assert_eq!(tags[0].name, "alpha");
            }
            Err(_) => {
                let fetched = web_scraping.get_api_tracker(tracker.id).await?;
                assert!(fetched.is_none());
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn update_api_tracker_with_tags_rolls_back_on_invalid_tags(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let tag = db.insert_user_tag(user.id, "alpha", "primary").await?;

        let tracker = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag.id])
        .build();

        let web_scraping = db.web_scraping(user.id);
        web_scraping.insert_api_tracker(&tracker).await?;

        let updated = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "updated-name",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .build();
        let result = web_scraping
            .update_api_tracker(
                &updated,
                Some(vec![uuid!("00000000-0000-0000-0000-000000000099")]),
            )
            .await;
        assert!(result.is_err());

        let fetched = web_scraping.get_api_tracker(tracker.id).await?.unwrap();
        assert_eq!(fetched.name, "some-name");

        let tags_map = web_scraping.get_api_tracker_tags(&[tracker.id]).await?;
        assert_eq!(tags_map[&tracker.id].len(), 1);
        assert_eq!(tags_map[&tracker.id][0].name, "alpha");

        Ok(())
    }

    #[sqlx::test]
    async fn insert_api_tracker_with_tags_isolated_between_users(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        let tag_a = db.insert_user_tag(user_a.id, "alpha", "primary").await?;
        let tag_b = db.insert_user_tag(user_b.id, "beta", "danger").await?;

        let tracker_a = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "tracker-a",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000010")),
        )?
        .with_tag_ids(&[tag_a.id])
        .build();
        let tracker_b = MockApiTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000003"),
            "tracker-b",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000030")),
        )?
        .with_tag_ids(&[tag_b.id])
        .build();

        let tags_a = db
            .web_scraping(user_a.id)
            .insert_api_tracker(&tracker_a)
            .await?;
        let tags_b = db
            .web_scraping(user_b.id)
            .insert_api_tracker(&tracker_b)
            .await?;

        assert_eq!(tags_a.len(), 1);
        assert_eq!(tags_a[0].name, "alpha");
        assert_eq!(tags_b.len(), 1);
        assert_eq!(tags_b[0].name, "beta");

        let map_a = db
            .web_scraping(user_a.id)
            .get_api_tracker_tags(&[tracker_a.id])
            .await?;
        let map_b = db
            .web_scraping(user_b.id)
            .get_api_tracker_tags(&[tracker_b.id])
            .await?;
        assert_eq!(map_a[&tracker_a.id].len(), 1);
        assert_eq!(map_b[&tracker_b.id].len(), 1);
        assert_eq!(map_a[&tracker_a.id][0].name, "alpha");
        assert_eq!(map_b[&tracker_b.id][0].name, "beta");

        Ok(())
    }
}
