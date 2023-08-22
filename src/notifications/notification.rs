use crate::notifications::{NotificationContent, NotificationDestination, NotificationId};
use time::OffsetDateTime;

/// Defines a notification.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Notification {
    /// Unique id of the notification.
    pub id: NotificationId,
    /// The destination of the notification.
    pub destination: NotificationDestination,
    /// The content of the notification.
    pub content: NotificationContent,
    /// The time at which the notification is scheduled to be sent, in UTC.
    pub scheduled_at: OffsetDateTime,
}

impl Notification {
    /// Creates a new notification.
    pub fn new(
        destination: NotificationDestination,
        content: NotificationContent,
        scheduled_at: OffsetDateTime,
    ) -> Self {
        Self {
            id: NotificationId::empty(),
            destination,
            content,
            scheduled_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Notification, NotificationContent, NotificationDestination};
    use crate::notifications::NotificationId;
    use time::OffsetDateTime;

    #[test]
    fn new_notification() -> anyhow::Result<()> {
        assert_eq!(
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::String("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ),
            Notification {
                id: NotificationId::empty(),
                destination: NotificationDestination::User(123.try_into()?),
                content: NotificationContent::String("abc".to_string()),
                scheduled_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );
        Ok(())
    }
}
