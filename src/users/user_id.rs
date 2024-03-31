use serde::{Deserialize, Serialize};
use std::{ops::Deref, str::FromStr};
use uuid::Uuid;

/// Represents unique identifier of the user.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct UserId(Uuid);
impl UserId {
    /// Creates a new unique user share ID.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl From<Uuid> for UserId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<&UserId> for Uuid {
    fn from(value: &UserId) -> Self {
        value.0
    }
}

impl FromStr for UserId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserId;
    use uuid::{uuid, Uuid, Version};

    #[test]
    fn creation() {
        let user_id = UserId::new();
        let underlying_uuid = Uuid::from(&user_id);
        assert_eq!(underlying_uuid.get_version(), Some(Version::Random));
        assert!(!underlying_uuid.is_nil());
    }

    #[test]
    fn conversion() {
        assert_eq!(
            *UserId::from(uuid!("00000000-0000-0000-0000-000000000001")),
            uuid!("00000000-0000-0000-0000-000000000001")
        );

        assert_eq!(
            Uuid::from(&UserId::from(uuid!("00000000-0000-0000-0000-000000000001"))),
            uuid!("00000000-0000-0000-0000-000000000001")
        );
    }

    #[test]
    fn parsing() -> anyhow::Result<()> {
        assert_eq!(
            "00000000-0000-0000-0000-000000000001".parse::<UserId>()?,
            UserId::from(uuid!("00000000-0000-0000-0000-000000000001"))
        );

        Ok(())
    }
}
