use crate::scheduler::{
    SchedulerJob, SchedulerJobMetadata, database_ext::RawSchedulerJobStoredData,
};
use tokio_cron_scheduler::Job;

pub trait JobExt {
    /// Populates job's `extra` field with the job metadata that includes type.
    fn set_job_type(&mut self, job_type: SchedulerJob) -> anyhow::Result<()>;

    /// Compares the schedules of the job and the raw job data.
    fn are_schedules_equal(
        &mut self,
        raw_job_data: &RawSchedulerJobStoredData,
    ) -> anyhow::Result<bool>;

    /// Populates job's fields with the raw job data.
    fn set_raw_job_data(&mut self, raw_job_data: RawSchedulerJobStoredData) -> anyhow::Result<()>;
}

impl JobExt for Job {
    /// Populates job's `extra` field with the job metadata that includes type.
    fn set_job_type(&mut self, job_type: SchedulerJob) -> anyhow::Result<()> {
        let mut job_data = self.job_data()?;
        job_data.extra = SchedulerJobMetadata::new(job_type).try_into()?;
        self.set_job_data(job_data)?;

        Ok(())
    }

    /// Compares the schedules of the job and the raw job data.
    fn are_schedules_equal(
        &mut self,
        raw_job_data: &RawSchedulerJobStoredData,
    ) -> anyhow::Result<bool> {
        Ok(raw_job_data.schedule
            == self
                .job_data()?
                .schedule()
                .map(|cron| cron.pattern.to_string()))
    }

    /// Populates job's fields with the raw job data.
    fn set_raw_job_data(&mut self, raw_job_data: RawSchedulerJobStoredData) -> anyhow::Result<()> {
        let mut job_data = self.job_data()?;
        job_data.id = Some(raw_job_data.id.into());
        job_data.last_updated = raw_job_data.last_updated.map(|t| t as u64);
        job_data.last_tick = raw_job_data.last_tick.map(|t| t as u64);
        job_data.next_tick = raw_job_data.next_tick.map(|t| t as u64).unwrap_or(0);
        job_data.job_type = raw_job_data.job_type;
        job_data.count = raw_job_data.count.map(|c| c as u32).unwrap_or(0);
        job_data.ran = raw_job_data.ran.unwrap_or(false);
        job_data.stopped = raw_job_data.stopped.unwrap_or(false);
        job_data.time_offset_seconds = raw_job_data.time_offset_seconds.unwrap_or(0);
        job_data.extra = raw_job_data.extra.unwrap_or_default();
        self.set_job_data(job_data)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JobExt;
    use crate::scheduler::{SchedulerJob, SchedulerJobMetadata};
    use std::time::Duration;
    use tokio_cron_scheduler::Job;

    #[tokio::test]
    async fn can_set_job_type() -> anyhow::Result<()> {
        let mut job = Job::new_one_shot(Duration::from_secs(10), |_, _| {})?;
        let original_job_data = job.job_data()?;
        assert!(original_job_data.extra.is_empty());

        job.set_job_type(SchedulerJob::NotificationsSend)?;

        let mut job_data = job.job_data()?;
        assert_eq!(
            SchedulerJobMetadata::try_from(job_data.extra.as_slice())?,
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend)
        );

        // Other fields should not be affected.
        job_data.extra.clone_from(&original_job_data.extra);
        assert_eq!(job_data, original_job_data);

        Ok(())
    }
}
