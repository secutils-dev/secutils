use crate::scheduler::SchedulerJob;
use serde::{Deserialize, Serialize};

/// Secutils.dev specific metadata of the scheduler job.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct SchedulerJobMetadata {
    /// The type of the job.
    pub job_type: SchedulerJob,
}

impl SchedulerJobMetadata {
    /// Create a new job state without retry state.
    pub fn new(job_type: SchedulerJob) -> Self {
        Self { job_type }
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
    use crate::scheduler::SchedulerJobMetadata;
    use insta::assert_debug_snapshot;

    #[test]
    fn properly_creates_metadata() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend),
            SchedulerJobMetadata {
                job_type: SchedulerJob::NotificationsSend,
            }
        );

        Ok(())
    }

    #[test]
    fn serialize() -> anyhow::Result<()> {
        assert_eq!(
            Vec::try_from(SchedulerJobMetadata::new(SchedulerJob::NotificationsSend))?,
            vec![0]
        );

        Ok(())
    }

    #[test]
    fn deserialize() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJobMetadata::try_from([0].as_ref())?,
            SchedulerJobMetadata::new(SchedulerJob::NotificationsSend)
        );

        assert_debug_snapshot!(SchedulerJobMetadata::try_from([1].as_ref()), @r###"
        Err(
            SerdeDeCustom,
        )
        "###);

        Ok(())
    }
}
