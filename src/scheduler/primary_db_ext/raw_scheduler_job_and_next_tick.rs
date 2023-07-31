use tokio_cron_scheduler::JobAndNextTick;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawSchedulerJobAndNextTick {
    pub id: uuid::Uuid,
    pub last_tick: Option<i64>,
    pub next_tick: Option<i64>,
    pub job_type: i64,
}

impl From<RawSchedulerJobAndNextTick> for JobAndNextTick {
    fn from(raw_data: RawSchedulerJobAndNextTick) -> Self {
        JobAndNextTick {
            id: Some(raw_data.id.into()),
            last_tick: raw_data.last_tick.map(|ts| ts as u64),
            next_tick: raw_data.next_tick.unwrap_or_default() as u64,
            job_type: raw_data.job_type as i32,
        }
    }
}
