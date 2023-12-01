use crate::scheduler::SchedulerJobRetryStrategy;
use serde::{Deserialize, Serialize};

/// Represents a job configuration that can be scheduled.
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerJobConfig {
    /// Defines a schedule for the job.
    pub schedule: String,
    /// Defines a retry strategy for the job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_strategy: Option<SchedulerJobRetryStrategy>,
    /// Indicates whether the job result should result into a notification. If retry strategy is
    /// defined, the error notification will be sent only if the job fails after all the retries.
    pub notifications: bool,
}
