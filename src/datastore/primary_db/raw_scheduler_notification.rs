#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawSchedulerNotification {
    pub job_id: uuid::fmt::Hyphenated,
    pub extra: Option<Vec<u8>>,
}
