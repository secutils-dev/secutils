use crate::scheduler::{SchedulerJob, SchedulerJobRetryState};
use serde::{Deserialize, Serialize};

/// Secutils.dev specific metadata of the scheduler job.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct SchedulerJobMetadata {
    /// The type of the job.
    pub job_type: SchedulerJob,
    /// The state of the job if it is being retried.
    pub retry: Option<SchedulerJobRetryState>,
}

impl SchedulerJobMetadata {
    /// Create a new job state without retry state.
    pub fn new(job_type: SchedulerJob) -> Self {
        Self {
            job_type,
            retry: None,
        }
    }
}

impl TryFrom<&[u8]> for SchedulerJobMetadata {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(postcard::from_bytes(value)?)
    }
}

impl TryFrom<SchedulerJobMetadata> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: SchedulerJobMetadata) -> Result<Self, Self::Error> {
        Ok(postcard::to_stdvec(&value)?)
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerJob;
    use crate::scheduler::{SchedulerJobMetadata, SchedulerJobRetryState};
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn properly_creates_metadata() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersSchedule),
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: None
            }
        );

        assert_eq!(
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend),
            SchedulerJobMetadata {
                job_type: SchedulerJob::NotificationsSend,
                retry: None
            }
        );

        Ok(())
    }

    #[test]
    fn serialize() -> anyhow::Result<()> {
        assert_eq!(
            Vec::try_from(SchedulerJobMetadata::new(
                SchedulerJob::WebPageTrackersSchedule
            ))?,
            vec![1, 0]
        );
        assert_eq!(
            Vec::try_from(SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 10,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                })
            })?,
            vec![1, 1, 10, 160, 31, 1, 10, 0, 0, 0, 0, 0, 0]
        );

        assert_eq!(
            Vec::try_from(SchedulerJobMetadata::new(SchedulerJob::NotificationsSend))?,
            vec![3, 0]
        );
        assert_eq!(
            Vec::try_from(SchedulerJobMetadata {
                job_type: SchedulerJob::NotificationsSend,
                retry: Some(SchedulerJobRetryState {
                    attempts: 10,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                })
            })?,
            vec![3, 1, 10, 160, 31, 1, 10, 0, 0, 0, 0, 0, 0]
        );

        Ok(())
    }

    #[test]
    fn deserialize() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJobMetadata::try_from([1, 0].as_ref())?,
            SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersSchedule)
        );

        assert_eq!(
            SchedulerJobMetadata::try_from([1, 1, 10, 160, 31, 1, 10, 0, 0, 0, 0, 0, 0].as_ref())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::WebPageTrackersSchedule,
                retry: Some(SchedulerJobRetryState {
                    attempts: 10,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                })
            }
        );

        assert_eq!(
            SchedulerJobMetadata::try_from([3, 0].as_ref())?,
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend)
        );

        assert_eq!(
            SchedulerJobMetadata::try_from([3, 1, 10, 160, 31, 1, 10, 0, 0, 0, 0, 0, 0].as_ref())?,
            SchedulerJobMetadata {
                job_type: SchedulerJob::NotificationsSend,
                retry: Some(SchedulerJobRetryState {
                    attempts: 10,
                    next_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                })
            }
        );

        assert_debug_snapshot!(SchedulerJobMetadata::try_from([4].as_ref()), @r###"
        Err(
            SerdeDeCustom,
        )
        "###);

        Ok(())
    }
}
