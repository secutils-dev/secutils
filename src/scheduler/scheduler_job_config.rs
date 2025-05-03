use serde::{Deserialize, Serialize};

/// Represents a job configuration that can be scheduled.
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerJobConfig {
    /// Defines a schedule for the job.
    pub schedule: String,
}
