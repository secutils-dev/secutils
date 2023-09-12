use crate::notifications::EmailNotificationContent;
use serde::{Deserialize, Serialize};

/// Describes the content of a notification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContent {
    /// Notification content is represented as a string.
    Text(String),
    /// Notification content is represented as an email.
    Email(EmailNotificationContent),
}

#[cfg(test)]
mod tests {
    use super::{EmailNotificationContent, NotificationContent};

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationContent::Text("abc".to_string()))?,
            vec![0, 3, 97, 98, 99]
        );

        assert_eq!(
            postcard::to_stdvec(&NotificationContent::Email(EmailNotificationContent::text(
                "abc", "def"
            )))?,
            vec![1, 3, 97, 98, 99, 3, 100, 101, 102, 0, 0]
        );
        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<NotificationContent>(&[0, 3, 97, 98, 99])?,
            NotificationContent::Text("abc".to_string())
        );

        assert_eq!(
            postcard::from_bytes::<NotificationContent>(&[
                1, 3, 97, 98, 99, 3, 100, 101, 102, 0, 0
            ])?,
            NotificationContent::Email(EmailNotificationContent::text("abc", "def"))
        );
        Ok(())
    }
}
