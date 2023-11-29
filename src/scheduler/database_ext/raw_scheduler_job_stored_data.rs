use tokio_cron_scheduler::{CronJob, JobStored, JobStoredData, JobType, NonCronJob};
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawSchedulerJobStoredData {
    pub id: Vec<u8>,
    pub last_updated: Option<i64>,
    pub last_tick: Option<i64>,
    pub next_tick: Option<i64>,
    pub job_type: i64,
    pub count: Option<i64>,
    pub ran: Option<i64>,
    pub stopped: Option<i64>,
    pub schedule: Option<String>,
    pub repeating: Option<i64>,
    pub repeated_every: Option<i64>,
    pub extra: Option<Vec<u8>>,
}

impl TryFrom<RawSchedulerJobStoredData> for JobStoredData {
    type Error = anyhow::Error;

    fn try_from(raw_data: RawSchedulerJobStoredData) -> Result<Self, Self::Error> {
        let job_type = JobType::from_i32(raw_data.job_type as i32);
        let job = match job_type {
            Some(JobType::Cron) => raw_data
                .schedule
                .map(|schedule| JobStored::CronJob(CronJob { schedule })),
            Some(JobType::Repeated | JobType::OneShot) => Some(JobStored::NonCronJob(NonCronJob {
                repeating: raw_data.repeating.unwrap_or_default() > 0,
                repeated_every: raw_data
                    .repeated_every
                    .map(|i| i as u64)
                    .unwrap_or_default(),
            })),
            None => None,
        };

        Ok(JobStoredData {
            id: Some(Uuid::from_slice(raw_data.id.as_slice())?.into()),
            last_updated: raw_data.last_updated.map(|ts| ts as u64),
            last_tick: raw_data.last_tick.map(|ts| ts as u64),
            next_tick: raw_data.next_tick.unwrap_or_default() as u64,
            job_type: raw_data.job_type as i32,
            count: raw_data.count.unwrap_or_default() as u32,
            extra: raw_data.extra.unwrap_or_default(),
            ran: raw_data.ran.unwrap_or_default() > 0,
            stopped: raw_data.stopped.unwrap_or_default() > 0,
            job,
        })
    }
}

impl TryFrom<&JobStoredData> for RawSchedulerJobStoredData {
    type Error = anyhow::Error;

    fn try_from(data: &JobStoredData) -> Result<Self, Self::Error> {
        let id: Uuid = if let Some(id) = &data.id {
            id.into()
        } else {
            return Err(anyhow::anyhow!("The job doesn't have UUID."));
        };

        let (repeating, repeated_every) = match data.job.as_ref() {
            Some(JobStored::NonCronJob(ct)) => {
                (Some(ct.repeating as i64), Some(ct.repeated_every as i64))
            }
            _ => (None, None),
        };

        Ok(RawSchedulerJobStoredData {
            id: id.into(),
            last_updated: data.last_updated.as_ref().map(|i| *i as i64),
            last_tick: data.last_tick.map(|ts| ts as i64),
            next_tick: Some(data.next_tick as i64),
            job_type: data.job_type as i64,
            count: Some(data.count as i64),
            extra: if data.extra.is_empty() {
                None
            } else {
                Some(data.extra.clone())
            },
            ran: Some(if data.ran { 1 } else { 0 }),
            stopped: Some(if data.stopped { 1 } else { 0 }),
            schedule: match data.job.as_ref() {
                Some(JobStored::CronJob(ct)) => Some(ct.schedule.clone()),
                _ => None,
            },
            repeating,
            repeated_every,
        })
    }
}
