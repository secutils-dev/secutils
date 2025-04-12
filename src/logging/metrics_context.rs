use serde::Serialize;
use serde_with::{DurationNanoSeconds, serde_as};
use std::time::Duration;

/// Represents a metrics context for the structured logging.
#[serde_as]
#[derive(Serialize, Default, Debug, Copy, Clone, PartialEq)]
pub struct MetricsContext {
    /// Script execution time in nanoseconds.
    #[serde_as(as = "Option<DurationNanoSeconds<u64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    script_execution_time: Option<Duration>,

    /// A number of times the job has been retried.
    #[serde(skip_serializing_if = "Option::is_none")]
    job_retries: Option<u32>,

    /// Job execution time in nanoseconds.
    #[serde_as(as = "Option<DurationNanoSeconds<u64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    job_execution_time: Option<Duration>,
}

impl MetricsContext {
    /// Adds script execution time to the metrics context.
    pub fn with_script_execution_time(self, script_execution_time: Duration) -> Self {
        Self {
            script_execution_time: Some(script_execution_time),
            ..self
        }
    }

    /// Adds job retries to the metrics context.
    pub fn with_job_retries(self, job_retries: u32) -> Self {
        Self {
            job_retries: Some(job_retries),
            ..self
        }
    }

    /// Adds job execution time to the metrics context.
    pub fn with_job_execution_time(self, job_execution_time: Duration) -> Self {
        Self {
            job_execution_time: Some(job_execution_time),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::logging::MetricsContext;
    use insta::assert_json_snapshot;
    use std::time::Duration;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let metrics = MetricsContext::default();
        assert_json_snapshot!(metrics, @"{}");

        let metrics = MetricsContext::default()
            .with_script_execution_time(Duration::from_secs(1))
            .with_job_execution_time(Duration::from_secs(2))
            .with_job_retries(3);
        assert_json_snapshot!(metrics, @r###"
        {
          "script_execution_time": 1000000000,
          "job_retries": 3,
          "job_execution_time": 2000000000
        }
        "###);

        Ok(())
    }
}
