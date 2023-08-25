mod notification_email_content;

use serde::{Deserialize, Serialize};

pub use self::notification_email_content::NotificationEmailContent;

/// Describes the content of a notification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContent {
    /// Notification content is represented as a string.
    Text(String),
    /// Notification content is represented as an email.
    Email(NotificationEmailContent),
}

#[cfg(test)]
mod tests {
    use super::{NotificationContent, NotificationEmailContent};

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationContent::Text("abc".to_string()))?,
            vec![0, 3, 97, 98, 99]
        );

        assert_eq!(
            postcard::to_stdvec(&NotificationContent::Email(NotificationEmailContent::text(
                "abc", "def"
            )))?,
            vec![1, 3, 97, 98, 99, 3, 100, 101, 102, 0]
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
            postcard::from_bytes::<NotificationContent>(&[1, 3, 97, 98, 99, 3, 100, 101, 102, 0])?,
            NotificationContent::Email(NotificationEmailContent::text("abc", "def"))
        );
        Ok(())
    }
}
