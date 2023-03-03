use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct UserData<V> {
    pub value: V,
    pub timestamp: OffsetDateTime,
}

impl<V> UserData<V> {
    pub fn new(value: V, timestamp: OffsetDateTime) -> Self {
        Self { value, timestamp }
    }
}
