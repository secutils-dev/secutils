use crate::users::UserId;
use serde::{Deserialize, Serialize};

/// Defines a notification destination.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
pub enum NotificationDestination {
    /// Notification will be sent to the user via default communication channel.
    User(UserId),
    /// Notification will be logged in the server log.
    ServerLog,
}

#[cfg(test)]
mod tests {
    use super::NotificationDestination;
    use crate::users::UserId;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationDestination::User(UserId(123)))?,
            vec![0, 246, 1]
        );
        assert_eq!(
            postcard::to_stdvec(&NotificationDestination::ServerLog)?,
            vec![1]
        );
        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<NotificationDestination>(&[0, 246, 1])?,
            NotificationDestination::User(UserId(123))
        );
        assert_eq!(
            postcard::from_bytes::<NotificationDestination>(&[1])?,
            NotificationDestination::ServerLog
        );
        Ok(())
    }
}
