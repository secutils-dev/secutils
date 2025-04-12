mod raw_notification;

use crate::{
    database::Database,
    notifications::{
        Notification, NotificationId, database_ext::raw_notification::RawNotification,
    },
};
use anyhow::bail;
use async_stream::try_stream;
use futures::Stream;
use sqlx::{query, query_as, query_scalar};
use time::OffsetDateTime;

/// Extends primary database with the notification-related methods.
impl Database {
    /// Retrieves notification from the database using ID.
    pub async fn get_notification(
        &self,
        id: NotificationId,
    ) -> anyhow::Result<Option<Notification>> {
        let id = *id;
        query_as!(
            RawNotification,
            r#"SELECT * FROM notifications WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(Notification::try_from)
        .transpose()
    }

    /// Inserts a new notification to the database.
    pub async fn insert_notification(
        &self,
        notification: &Notification,
    ) -> anyhow::Result<NotificationId> {
        if !notification.id.is_empty() {
            bail!("Notification ID must be empty for insertion.");
        }

        let raw_notification = RawNotification::try_from(notification)?;
        let id = query_scalar!(
            r#"INSERT INTO notifications (destination, content, scheduled_at) VALUES ($1, $2, $3) RETURNING id"#,
            raw_notification.destination,
            raw_notification.content,
            raw_notification.scheduled_at
        ).fetch_one(&self.pool)
        .await?;

        NotificationId::try_from(id)
    }

    /// Removes notification from the database using notification ID.
    pub async fn remove_notification(&self, id: NotificationId) -> anyhow::Result<()> {
        if id.is_empty() {
            bail!("Notification ID must not be empty for removal.");
        }

        query!(r#"DELETE FROM notifications WHERE id = $1"#, *id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Retrieves a list of notification IDs that are scheduled at or before specified date.
    pub fn get_notification_ids(
        &self,
        scheduled_before_or_at: OffsetDateTime,
        page_size: usize,
    ) -> impl Stream<Item = anyhow::Result<NotificationId>> + '_ {
        let page_limit = page_size as i64;
        try_stream! {
            let mut last_id = 0;
            let mut conn = self.pool.acquire().await?;
            loop {
                 let raw_notification_ids = query!(
                    r#"SELECT id FROM notifications WHERE scheduled_at <= $1 AND id > $2 ORDER BY scheduled_at, id LIMIT $3;"#,
                    scheduled_before_or_at,
                    last_id,
                    page_limit
                ).fetch_all(&mut *conn).await?;

                let is_last_page = raw_notification_ids.len() < page_size;
                for raw_notification_id in raw_notification_ids {
                    last_id = raw_notification_id.id;
                    yield NotificationId::try_from(raw_notification_id.id)?;
                }

                if is_last_page {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        notifications::{Notification, NotificationContent, NotificationDestination},
    };
    use futures::StreamExt;
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_add_and_retrieve_notifications(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        assert!(db.get_notification(1.try_into()?).await?.is_none());

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        ];

        for notification in notifications {
            db.insert_notification(&notification).await?;
        }

        assert_debug_snapshot!(db.get_notification(1.try_into()?).await?, @r###"
        Some(
            Notification {
                id: NotificationId(
                    1,
                ),
                destination: User(
                    UserId(
                        00000000-0000-0000-0000-000000000001,
                    ),
                ),
                content: Text(
                    "abc",
                ),
                scheduled_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_notification(2.try_into()?).await?, @r###"
        Some(
            Notification {
                id: NotificationId(
                    2,
                ),
                destination: User(
                    UserId(
                        00000000-0000-0000-0000-000000000001,
                    ),
                ),
                content: Text(
                    "abc",
                ),
                scheduled_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_notification(3.try_into()?).await?, @"None");

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_notifications(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        ];

        for notification in notifications {
            db.insert_notification(&notification).await?;
        }

        assert!(db.get_notification(1.try_into()?).await?.is_some());
        assert!(db.get_notification(2.try_into()?).await?.is_some());

        db.remove_notification(1.try_into()?).await?;

        assert!(db.get_notification(1.try_into()?).await?.is_none());
        assert!(db.get_notification(2.try_into()?).await?.is_some());

        db.remove_notification(2.try_into()?).await?;

        assert!(db.get_notification(1.try_into()?).await?.is_none());
        assert!(db.get_notification(2.try_into()?).await?.is_none());

        assert!(db.get_notification(3.try_into()?).await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_get_notification_ids(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        let scheduled_before_or_at = OffsetDateTime::from_unix_timestamp(946720710)?;

        let notifications = db.get_notification_ids(scheduled_before_or_at, 2);
        assert_eq!(notifications.collect::<Vec<_>>().await.len(), 0);

        for n in 0..=19 {
            db.insert_notification(&Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
                NotificationContent::Text(format!("abc{}", n)),
                OffsetDateTime::from_unix_timestamp(946720700 + n)?,
            ))
            .await?;
        }

        let notification_ids = db
            .get_notification_ids(scheduled_before_or_at, 2)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(notification_ids.len(), 11);

        assert_debug_snapshot!(notification_ids
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?, @r###"
        [
            NotificationId(
                1,
            ),
            NotificationId(
                2,
            ),
            NotificationId(
                3,
            ),
            NotificationId(
                4,
            ),
            NotificationId(
                5,
            ),
            NotificationId(
                6,
            ),
            NotificationId(
                7,
            ),
            NotificationId(
                8,
            ),
            NotificationId(
                9,
            ),
            NotificationId(
                10,
            ),
            NotificationId(
                11,
            ),
        ]
        "###);

        Ok(())
    }
}
