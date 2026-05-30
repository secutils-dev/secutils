use serde::{Deserialize, Serialize};

/// Represents a job that can be scheduled.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum SchedulerJob {
    NotificationsSend,
    WebhooksKvSweep,
}

impl SchedulerJob {
    /// Indicates whether the job should be scheduled only once.
    pub fn is_unique(&self) -> bool {
        match self {
            Self::NotificationsSend => true,
            Self::WebhooksKvSweep => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerJob;

    #[test]
    fn properly_determines_unique_jobs() -> anyhow::Result<()> {
        assert!(SchedulerJob::NotificationsSend.is_unique());
        assert!(SchedulerJob::WebhooksKvSweep.is_unique());

        Ok(())
    }
}
