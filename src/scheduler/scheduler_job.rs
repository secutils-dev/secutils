use serde::{Deserialize, Serialize};

/// Represents a job that can be scheduled.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum SchedulerJob {
    NotificationsSend,
}

impl SchedulerJob {
    /// Indicates whether the job should be scheduled only once.
    pub fn is_unique(&self) -> bool {
        match self {
            Self::NotificationsSend => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerJob;

    #[test]
    fn properly_determines_unique_jobs() -> anyhow::Result<()> {
        assert!(SchedulerJob::NotificationsSend.is_unique());

        Ok(())
    }
}
