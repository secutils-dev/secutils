use crate::notifications::Notification;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawNotification {
    pub id: i64,
    pub destination: Vec<u8>,
    pub content: Vec<u8>,
    pub scheduled_at: i64,
}

impl TryFrom<RawNotification> for Notification {
    type Error = anyhow::Error;

    fn try_from(raw_notification: RawNotification) -> Result<Self, Self::Error> {
        Ok(Notification {
            id: raw_notification.id.try_into()?,
            destination: postcard::from_bytes(&raw_notification.destination)?,
            content: postcard::from_bytes(&raw_notification.content)?,
            scheduled_at: OffsetDateTime::from_unix_timestamp(raw_notification.scheduled_at)?,
        })
    }
}

impl TryFrom<&Notification> for RawNotification {
    type Error = anyhow::Error;

    fn try_from(notification: &Notification) -> Result<Self, Self::Error> {
        Ok(RawNotification {
            id: *notification.id,
            destination: postcard::to_stdvec(&notification.destination)?,
            content: postcard::to_stdvec(&notification.content)?,
            scheduled_at: notification.scheduled_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawNotification;
    use crate::notifications::{Notification, NotificationContent, NotificationDestination};
    use time::OffsetDateTime;

    #[test]
    fn can_convert_to_notification() -> anyhow::Result<()> {
        assert_eq!(
            Notification::try_from(RawNotification {
                id: 1,
                destination: vec![0, 246, 1],
                content: vec![0, 3, 97, 98, 99],
                scheduled_at: 946720800,
            })?,
            Notification {
                id: 1.try_into()?,
                destination: NotificationDestination::User(123.try_into()?),
                content: NotificationContent::String("abc".to_string()),
                scheduled_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_to_raw_notification() -> anyhow::Result<()> {
        assert_eq!(
            RawNotification::try_from(&Notification {
                id: 1.try_into()?,
                destination: NotificationDestination::User(123.try_into()?),
                content: NotificationContent::String("abc".to_string()),
                scheduled_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawNotification {
                id: 1,
                destination: vec![0, 246, 1],
                content: vec![0, 3, 97, 98, 99],
                scheduled_at: 946720800,
            }
        );

        Ok(())
    }
}
