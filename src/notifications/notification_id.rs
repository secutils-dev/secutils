use serde::{Deserialize, Serialize};
use std::ops::Deref;

/// Defines a type for the ID of the notification.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct NotificationId(i32);
impl NotificationId {
    /// Creates a new notification ID. Use this only to create a new notification.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if the notification ID is empty.
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl TryFrom<i32> for NotificationId {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(anyhow::anyhow!("Notification ID must be greater than 0."))
        }
    }
}

impl Deref for NotificationId {
    type Target = i32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::NotificationId;

    #[test]
    fn empty() {
        assert_eq!(*NotificationId::empty(), 0);
    }

    #[test]
    fn is_empty() -> anyhow::Result<()> {
        assert!(NotificationId::empty().is_empty());
        assert!(!NotificationId::try_from(1)?.is_empty());

        Ok(())
    }

    #[test]
    fn conversion() -> anyhow::Result<()> {
        assert_eq!(*NotificationId::try_from(1)?, 1);
        assert_eq!(*NotificationId::try_from(100)?, 100);

        assert!(NotificationId::try_from(-1).is_err());
        assert!(NotificationId::try_from(0).is_err());

        Ok(())
    }
}
