use anyhow::anyhow;

/// Represents a job that can be scheduled.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum SchedulerJob {
    ResourcesTrackersTrigger = 0,
    ResourcesTrackersDispatch,
}

impl SchedulerJob {
    /// Indicates whether the job should be scheduled only once.
    pub fn is_unique(&self) -> bool {
        match self {
            Self::ResourcesTrackersDispatch => true,
            Self::ResourcesTrackersTrigger => false,
        }
    }
}

impl TryFrom<u8> for SchedulerJob {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ResourcesTrackersTrigger),
            1 => Ok(Self::ResourcesTrackersDispatch),
            num => Err(anyhow!("Unknown job type: {}", num)),
        }
    }
}

impl TryFrom<&[u8]> for SchedulerJob {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != 1 {
            Err(anyhow!(
                "Serialized job type should be exactly 1 byte, but got {}",
                value.len()
            ))
        } else {
            Self::try_from(value[0])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerJob;
    use insta::assert_debug_snapshot;

    #[test]
    fn properly_determines_unique_jobs() -> anyhow::Result<()> {
        assert!(!SchedulerJob::ResourcesTrackersTrigger.is_unique());
        assert!(SchedulerJob::ResourcesTrackersDispatch.is_unique());

        Ok(())
    }

    #[test]
    fn can_parse_u8() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJob::try_from(0).ok(),
            Some(SchedulerJob::ResourcesTrackersTrigger)
        );
        assert_eq!(
            SchedulerJob::try_from(1).ok(),
            Some(SchedulerJob::ResourcesTrackersDispatch)
        );

        assert_debug_snapshot!(SchedulerJob::try_from(2), @r###"
        Err(
            "Unknown job type: 2",
        )
        "###);

        Ok(())
    }

    #[test]
    fn can_parse_vec_slice() -> anyhow::Result<()> {
        assert_eq!(
            SchedulerJob::try_from([0].as_slice()).ok(),
            Some(SchedulerJob::ResourcesTrackersTrigger)
        );
        assert_eq!(
            SchedulerJob::try_from([1].as_slice()).ok(),
            Some(SchedulerJob::ResourcesTrackersDispatch)
        );

        assert_debug_snapshot!(SchedulerJob::try_from([].as_slice()), @r###"
        Err(
            "Serialized job type should be exactly 1 byte, but got 0",
        )
        "###);
        assert_debug_snapshot!(SchedulerJob::try_from([2].as_slice()), @r###"
        Err(
            "Unknown job type: 2",
        )
        "###);
        assert_debug_snapshot!(SchedulerJob::try_from([0, 1].as_slice()), @r###"
        Err(
            "Serialized job type should be exactly 1 byte, but got 2",
        )
        "###);

        Ok(())
    }
}