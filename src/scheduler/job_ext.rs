use crate::scheduler::{SchedulerJob, SchedulerJobMetadata};
use tokio_cron_scheduler::{Job, JobStoredData};

pub trait JobExt {
    /// Populates job's `extra` field with the job metadata that includes type.
    fn set_job_type(&mut self, job_type: SchedulerJob) -> anyhow::Result<()>;
}

impl JobExt for Job {
    /// Populates job's `extra` field with the job metadata that includes type.
    fn set_job_type(&mut self, job_type: SchedulerJob) -> anyhow::Result<()> {
        let job_data = self.job_data()?;
        self.set_job_data(JobStoredData {
            extra: SchedulerJobMetadata::new(job_type).try_into()?,
            ..job_data
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JobExt;
    use crate::scheduler::{SchedulerJob, SchedulerJobMetadata};
    use std::time::Duration;
    use tokio_cron_scheduler::{Job, JobStoredData};

    #[tokio::test]
    async fn can_set_job_type() -> anyhow::Result<()> {
        let mut job = Job::new_one_shot(Duration::from_secs(10), |_, _| {})?;
        let original_job_data = job.job_data()?;
        assert!(original_job_data.extra.is_empty());

        job.set_job_type(SchedulerJob::WebPageTrackersSchedule)?;

        let job_data = job.job_data()?;
        assert_eq!(
            SchedulerJobMetadata::try_from(job_data.extra.as_slice())?,
            SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersSchedule)
        );

        job.set_job_type(SchedulerJob::NotificationsSend)?;

        let job_data = job.job_data()?;
        assert_eq!(
            SchedulerJobMetadata::try_from(job_data.extra.as_slice())?,
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend)
        );

        // Other fields should not be affected.
        assert_eq!(
            job_data,
            JobStoredData {
                extra: Vec::try_from(SchedulerJobMetadata::new(SchedulerJob::NotificationsSend))?,
                ..original_job_data
            }
        );

        Ok(())
    }
}
