use serde::{Deserialize, Serialize};

/// Describes the content of a notification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContent {
    /// Notification content is represented as a string.
    String(String),
}

#[cfg(test)]
mod tests {
    use super::NotificationContent;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationContent::String("abc".to_string()))?,
            vec![0, 3, 97, 98, 99]
        );
        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<NotificationContent>(&[0, 3, 97, 98, 99])?,
            NotificationContent::String("abc".to_string())
        );
        Ok(())
    }
}
