use crate::utils::web_scraping::WebPageTrackerKind;
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

#[cfg(test)]
mod tests {
    use super::SchedulerJob;
    use crate::utils::web_scraping::WebPageTrackerKind;

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
}
