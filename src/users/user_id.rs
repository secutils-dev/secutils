use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct UserId(i64);
impl UserId {
    pub const fn empty() -> Self {
        Self(0)
    }
}

impl TryFrom<i64> for UserId {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(anyhow::anyhow!("User ID must be greater than 0."))
        }
    }
}

impl Deref for UserId {
    type Target = i64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserId;

    #[test]
    fn empty() {
        assert_eq!(*UserId::empty(), 0);
    }

    #[test]
    fn conversion() -> anyhow::Result<()> {
        assert_eq!(*UserId::try_from(1)?, 1);
        assert_eq!(*UserId::try_from(100)?, 100);

        assert!(UserId::try_from(-1).is_err());
        assert!(UserId::try_from(0).is_err());

        Ok(())
    }
}
