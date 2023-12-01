use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Describes the state of a job that is being retried.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct SchedulerJobRetryState {
    /// How many times the job has been retried.
    pub attempts: u32,
    /// The time at which the job will be retried.
    pub next_at: OffsetDateTime,
}
