use crate::{
    api::Api,
    database::Database,
    network::DnsResolver,
    notifications::{Notification, NotificationContent, NotificationDestination, NotificationId},
};
use futures::{pin_mut, StreamExt};
use std::{borrow::Cow, cmp};
use time::OffsetDateTime;

/// Defines a maximum number of notifications that can be retrieved from the database at once.
const MAX_NOTIFICATIONS_PAGE_SIZE: usize = 100;

/// Describes the API to work with notifications.
pub struct NotificationsApi<'a> {
    db: Cow<'a, Database>,
}

impl<'a> NotificationsApi<'a> {
    /// Creates Notifications API.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db: Cow::Borrowed(db),
        }
    }

    /// Schedules a new notification.
    pub async fn schedule_notification(
        &self,
        destination: NotificationDestination,
        content: NotificationContent,
        scheduled_at: OffsetDateTime,
    ) -> anyhow::Result<NotificationId> {
        self.db
            .insert_notification(&Notification::new(destination, content, scheduled_at))
            .await
    }

    /// Sends pending notifications. The max number to send is limited by `limit`.
    pub async fn send_pending_notifications(&self, limit: usize) -> anyhow::Result<usize> {
        let pending_notification_ids = self.db.get_notification_ids(
            OffsetDateTime::now_utc(),
            cmp::min(MAX_NOTIFICATIONS_PAGE_SIZE, limit),
        );
        pin_mut!(pending_notification_ids);

        let mut sent_notifications = 0;
        while let Some(notification_id) = pending_notification_ids.next().await {
            if let Some(notification) = self.db.get_notification(notification_id?).await? {
                if let Err(err) = self.send_notification(&notification).await {
                    log::error!(
                        "Failed to send notification {}: {:?}",
                        *notification.id,
                        err
                    );
                } else {
                    sent_notifications += 1;
                    self.db.remove_notification(notification.id).await?;
                }
            }

            if sent_notifications >= limit {
                break;
            }
        }

        Ok(sent_notifications)
    }

    /// Sends notification and removes it from the database, if it was sent successfully.
    async fn send_notification(&self, notification: &Notification) -> anyhow::Result<()> {
        match notification.destination {
            NotificationDestination::User(user_id) => {
                log::info!("Sending notification to {:?}: {:?}", user_id, notification);
            }
            NotificationDestination::ServerLog => {
                log::info!("Sending notification: {:?}", notification);
            }
        }

        Ok(())
    }
}

impl<DR: DnsResolver> Api<DR> {
    /// Returns an API to work with notifications.
    pub fn notifications(&self) -> NotificationsApi<'_> {
        NotificationsApi::new(&self.db)
    }
}

#[cfg(test)]
mod tests {
    use super::NotificationsApi;
    use crate::{
        database::Database,
        notifications::{Notification, NotificationContent, NotificationDestination},
        tests::{mock_db, mock_user},
        users::User,
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    async fn initialize_mock_db(user: &User) -> anyhow::Result<Database> {
        let db = mock_db().await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn properly_schedules_notification() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = NotificationsApi::new(&mock_db);

        assert!(mock_db.get_notification(1.try_into()?).await?.is_none());

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::String("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::String("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        ];

        for notification in notifications.into_iter() {
            api.schedule_notification(
                notification.destination,
                notification.content,
                notification.scheduled_at,
            )
            .await?;
        }

        assert_debug_snapshot!(mock_db.get_notification(1.try_into()?).await?, @r###"
        Some(
            Notification {
                id: NotificationId(
                    1,
                ),
                destination: User(
                    UserId(
                        123,
                    ),
                ),
                content: String(
                    "abc",
                ),
                scheduled_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(mock_db.get_notification(2.try_into()?).await?, @r###"
        Some(
            Notification {
                id: NotificationId(
                    2,
                ),
                destination: User(
                    UserId(
                        123,
                    ),
                ),
                content: String(
                    "abc",
                ),
                scheduled_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(mock_db.get_notification(3.try_into()?).await?, @"None");

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_sends_all_pending_notifications() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = NotificationsApi::new(&mock_db);

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::String("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720700)?,
            ),
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::String("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        ];

        for notification in notifications.into_iter() {
            api.schedule_notification(
                notification.destination,
                notification.content,
                notification.scheduled_at,
            )
            .await?;
        }

        assert!(mock_db.get_notification(1.try_into()?).await?.is_some());
        assert!(mock_db.get_notification(2.try_into()?).await?.is_some());

        assert_eq!(api.send_pending_notifications(3).await?, 2);

        assert!(mock_db.get_notification(1.try_into()?).await?.is_none());
        assert!(mock_db.get_notification(2.try_into()?).await?.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_sends_pending_notifications_in_batches() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = NotificationsApi::new(&mock_db);

        for n in 0..=9 {
            api.schedule_notification(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::String(format!("{}", n)),
                OffsetDateTime::from_unix_timestamp(946720800 + n)?,
            )
            .await?;
        }

        for n in 0..=9 {
            assert!(mock_db
                .get_notification((n + 1).try_into()?)
                .await?
                .is_some());
        }

        assert_eq!(api.send_pending_notifications(3).await?, 3);

        for n in 0..=9 {
            assert_eq!(
                mock_db
                    .get_notification((n + 1).try_into()?)
                    .await?
                    .is_some(),
                n >= 3
            );
        }

        assert_eq!(api.send_pending_notifications(3).await?, 3);

        for n in 0..=9 {
            assert_eq!(
                mock_db
                    .get_notification((n + 1).try_into()?)
                    .await?
                    .is_some(),
                n >= 6
            );
        }

        assert_eq!(api.send_pending_notifications(10).await?, 4);

        for n in 0..=9 {
            assert!(mock_db
                .get_notification((n + 1).try_into()?)
                .await?
                .is_none());
        }

        Ok(())
    }
}
