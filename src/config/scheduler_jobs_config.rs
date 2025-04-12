use serde_derive::{Deserialize, Serialize};

/// Configuration for the Secutils.dev scheduler jobs.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `WebPageTrackersSchedule` job.
    pub web_page_trackers_schedule: String,
    /// The schedule to use for the `WebPageTrackersFetch` job.
    pub web_page_trackers_fetch: String,
    /// The schedule to use for the `NotificationsSend` job.
    pub notifications_send: String,
}

impl Default for SchedulerJobsConfig {
    fn default() -> Self {
        Self {
            web_page_trackers_schedule: "0 * * * * *".to_string(),
            web_page_trackers_fetch: "0 * * * * *".to_string(),
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
        assert_toml_snapshot!(SchedulerJobsConfig::default(), @r###"
        web_page_trackers_schedule = '0 * * * * *'
        web_page_trackers_fetch = '0 * * * * *'
        notifications_send = '0/30 * * * * *'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SchedulerJobsConfig = toml::from_str(
            r#"
        web_page_trackers_schedule = '0 * * * * *'
        web_page_trackers_fetch = '0 * * * * *'
        notifications_send = '0/30 * * * * *'
    "#,
        )
        .unwrap();
        assert_eq!(config, SchedulerJobsConfig::default());
    }
}
