use serde::{Deserialize, Serialize};
use std::{ops::Deref, str::FromStr};
use uuid::Uuid;

/// Represents unique identifier of the shared resource.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct UserShareId(Uuid);
impl UserShareId {
    /// Creates a new unique user share ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserShareId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for UserShareId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<&UserShareId> for Uuid {
    fn from(value: &UserShareId) -> Self {
        value.0
    }
}

impl FromStr for UserShareId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Deref for UserShareId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserShareId;
    use uuid::{uuid, Uuid, Version};

    #[test]
    fn creation() {
        let user_share_id = UserShareId::new();
        let underlying_uuid = Uuid::from(&user_share_id);
        assert_eq!(underlying_uuid.get_version(), Some(Version::Random));
        assert!(!underlying_uuid.is_nil());

        let user_share_id = UserShareId::default();
        let underlying_uuid = Uuid::from(&user_share_id);
        assert_eq!(underlying_uuid.get_version(), Some(Version::Random));
        assert!(!underlying_uuid.is_nil());
    }

    #[test]
    fn conversion() {
        assert_eq!(
            *UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
            uuid!("00000000-0000-0000-0000-000000000001")
        );

        assert_eq!(
            Uuid::from(&UserShareId::from(uuid!(
                "00000000-0000-0000-0000-000000000001"
            ))),
            uuid!("00000000-0000-0000-0000-000000000001")
        );
    }

    #[test]
    fn parsing() -> anyhow::Result<()> {
        assert_eq!(
            "00000000-0000-0000-0000-000000000001".parse::<UserShareId>()?,
            UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001"))
        );

        Ok(())
    }
}
