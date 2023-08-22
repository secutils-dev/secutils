use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct UserId(pub i64);
impl UserId {
    pub const fn empty() -> Self {
        Self(-1)
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserId;

    #[test]
    fn empty() {
        assert_eq!(UserId::empty(), UserId(-1));
    }
}
