use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RawSchedulerJobStoredData {
    pub id: Uuid,
    pub last_updated: Option<i64>,
    pub last_tick: Option<i64>,
    pub next_tick: Option<i64>,
    pub job_type: i32,
    pub count: Option<i32>,
    pub ran: Option<bool>,
    pub stopped: Option<bool>,
    pub schedule: Option<String>,
    pub repeating: Option<bool>,
    pub repeated_every: Option<i64>,
    pub extra: Option<Vec<u8>>,
    pub time_offset_seconds: Option<i32>,
}
