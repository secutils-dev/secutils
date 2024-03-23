use cron::Schedule;
use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::str::FromStr;

/// Configuration for the Secutils.dev scheduler jobs.
#[serde_as]
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `WebPageTrackersSchedule` job.
    #[serde_as(as = "DisplayFromStr")]
    pub web_page_trackers_schedule: Schedule,
    /// The schedule to use for the `WebPageTrackersFetch` job.
    #[serde_as(as = "DisplayFromStr")]
    pub web_page_trackers_fetch: Schedule,
    /// The schedule to use for the `NotificationsSend` job.
    #[serde_as(as = "DisplayFromStr")]
    pub notifications_send: Schedule,
}

impl Default for SchedulerJobsConfig {
    fn default() -> Self {
        Self {
            web_page_trackers_schedule: Schedule::from_str("0 * * * * * *")
                .expect("Cannot parse web page trackers schedule job schedule."),
            web_page_trackers_fetch: Schedule::from_str("0 * * * * * *")
                .expect("Cannot parse web page trackers fetch job schedule."),
            notifications_send: Schedule::from_str("0/30 * * * * * *")
                .expect("Cannot parse notifications send job schedule."),
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
        web-page-trackers-schedule = '0 * * * * * *'
        web-page-trackers-fetch = '0 * * * * * *'
        notifications-send = '0/30 * * * * * *'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SchedulerJobsConfig = toml::from_str(
            r#"
        web-page-trackers-schedule = '0 * * * * * *'
        web-page-trackers-fetch = '0 * * * * * *'
        notifications-send = '0/30 * * * * * *'
    "#,
        )
        .unwrap();
        assert_eq!(config, SchedulerJobsConfig::default());
    }
}
