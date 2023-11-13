use crate::utils::WebPageTrackerKind;
use serde::{Deserialize, Serialize};

/// Represents a job that can be scheduled.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum SchedulerJob {
    WebPageTrackersTrigger { kind: WebPageTrackerKind },
    WebPageTrackersSchedule,
    WebPageTrackersFetch,
    NotificationsSend,
}

impl SchedulerJob {
    /// Indicates whether the job should be scheduled only once.
    pub fn is_unique(&self) -> bool {
        match self {
            Self::WebPageTrackersSchedule => true,
            Self::WebPageTrackersTrigger { .. } => false,
            Self::WebPageTrackersFetch => true,
            Self::NotificationsSend => true,
        }
    }
}

impl TryFrom<&[u8]> for SchedulerJob {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(postcard::from_bytes(value)?)
    }
}

impl TryFrom<SchedulerJob> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: SchedulerJob) -> Result<Self, Self::Error> {
        Ok(postcard::to_stdvec(&value)?)
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerJob;
    use crate::utils::WebPageTrackerKind;
    use insta::assert_debug_snapshot;

    #[test]
    fn properly_determines_unique_jobs() -> anyhow::Result<()> {
        assert!(!SchedulerJob::WebPageTrackersTrigger {
            kind: WebPageTrackerKind::WebPageContent
        }
        .is_unique());
        assert!(!SchedulerJob::WebPageTrackersTrigger {
            kind: WebPageTrackerKind::WebPageResources
        }
        .is_unique());
        assert!(SchedulerJob::WebPageTrackersSchedule.is_unique());
        assert!(SchedulerJob::WebPageTrackersFetch.is_unique());
        assert!(SchedulerJob::NotificationsSend.is_unique());

        Ok(())
    }

    #[test]
    fn serialize() -> anyhow::Result<()> {
        assert_eq!(
            Vec::try_from(SchedulerJob::WebPageTrackersTrigger {
                kind: WebPageTrackerKind::WebPageResources
            })?,
            vec![0, 0]
        );
        assert_eq!(
            Vec::try_from(SchedulerJob::WebPageTrackersTrigger {
                kind: WebPageTrackerKind::WebPageContent
            })?,
            vec![0, 1]
        );
        assert_eq!(
            Vec::try_from(SchedulerJob::WebPageTrackersSchedule)?,
            vec![1]
        );
        assert_eq!(Vec::try_from(SchedulerJob::WebPageTrackersFetch)?, vec![2]);
        assert_eq!(Vec::try_from(SchedulerJob::NotificationsSend)?, vec![3]);

        Ok(())
    }

    #[test]
    fn deserialize() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJob::try_from([0, 0].as_ref())?,
            SchedulerJob::WebPageTrackersTrigger {
                kind: WebPageTrackerKind::WebPageResources
            }
        );

        assert_eq!(
            SchedulerJob::try_from([0, 1].as_ref())?,
            SchedulerJob::WebPageTrackersTrigger {
                kind: WebPageTrackerKind::WebPageContent
            }
        );

        assert_eq!(
            SchedulerJob::try_from([1].as_ref())?,
            SchedulerJob::WebPageTrackersSchedule
        );

        assert_eq!(
            SchedulerJob::try_from([2].as_ref())?,
            SchedulerJob::WebPageTrackersFetch
        );

        assert_eq!(
            SchedulerJob::try_from([3].as_ref())?,
            SchedulerJob::NotificationsSend
        );

        assert_debug_snapshot!(SchedulerJob::try_from([4].as_ref()), @r###"
        Err(
            SerdeDeCustom,
        )
        "###);

        Ok(())
    }
}
