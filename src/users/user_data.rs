use crate::users::UserId;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct UserData<V> {
    pub user_id: UserId,
    pub key: Option<String>,
    pub value: V,
    pub timestamp: OffsetDateTime,
}

impl<V> UserData<V> {
    pub fn new(user_id: UserId, value: V, timestamp: OffsetDateTime) -> Self {
        Self {
            user_id,
            key: None,
            value,
            timestamp,
        }
    }
}
