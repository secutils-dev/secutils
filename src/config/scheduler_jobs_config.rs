use serde_derive::{Deserialize, Serialize};

/// Configuration for the Secutils.dev scheduler jobs.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `NotificationsSend` job.
    pub notifications_send: String,
}

impl Default for SchedulerJobsConfig {
    fn default() -> Self {
        Self {
            notifications_send: "0/30 * * * * *".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SchedulerJobsConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SchedulerJobsConfig::default(), @"notifications_send = '0/30 * * * * *'");
    }

    #[test]
    fn deserialization() {
        let config: SchedulerJobsConfig = toml::from_str(
            r#"
        notifications_send = '0/30 * * * * *'
    "#,
        )
        .unwrap();
        assert_eq!(config, SchedulerJobsConfig::default());
    }
}
