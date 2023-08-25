use crate::users::UserId;
use serde::{Deserialize, Serialize};

/// Defines a notification destination.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum NotificationDestination {
    /// Notification will be sent to the user via default communication channel.
    User(UserId),
    /// Notification will be sent to the specified email.
    Email(String),
    /// Notification will be logged in the server log.
    ServerLog,
}

#[cfg(test)]
mod tests {
    use super::NotificationDestination;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationDestination::User(123.try_into()?))?,
            vec![0, 246, 1]
        );
        assert_eq!(
            postcard::to_stdvec(&NotificationDestination::Email("abc".to_string()))?,
            vec![1, 3, 97, 98, 99]
        );
        assert_eq!(
            postcard::to_stdvec(&NotificationDestination::ServerLog)?,
            vec![2]
        );
        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<NotificationDestination>(&[0, 246, 1])?,
            NotificationDestination::User(123.try_into()?)
        );
        assert_eq!(
            postcard::from_bytes::<NotificationDestination>(&[1, 3, 97, 98, 99])?,
            NotificationDestination::Email("abc".to_string())
        );
        assert_eq!(
            postcard::from_bytes::<NotificationDestination>(&[2])?,
            NotificationDestination::ServerLog
        );
        Ok(())
    }
}
