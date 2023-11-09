use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::{collections::HashMap, time::Duration};

/// We currently support up to 10 revisions of the web page content.
pub const MAX_WEB_PAGE_TRACKER_REVISIONS: usize = 10;

/// We currently wait up to 60 seconds before starting to track web page.
pub const MAX_WEB_PAGE_TRACKER_DELAY: Duration = Duration::from_secs(60);

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageTrackerSettings {
    /// A number of revisions of the web page content to track.
    pub revisions: usize,
    /// Optional schedule to track web page on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Number of milliseconds to wait after web page enters "idle" state to start tracking.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub delay: Duration,
    /// Optional scripts to inject into the tracked web page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<HashMap<String, String>>,
    /// Indicates that web page change notifications are enabled for this tracker.
    pub enable_notifications: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        web_scraping::WebPageTrackerSettings, WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: None,
            delay: Duration::from_millis(2500),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "delay": 2500,
          "enableNotifications": true
        }
        "###);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2500),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "schedule": "0 0 * * *",
          "delay": 2500,
          "enableNotifications": true
        }
        "###);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2500),
            scripts: Some(
                [(
                    WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                    "return resource;".to_string(),
                )]
                .into_iter()
                .collect(),
            ),
            enable_notifications: true,
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "schedule": "0 0 * * *",
          "delay": 2500,
          "scripts": {
            "resourceFilterMap": "return resource;"
          },
          "enableNotifications": true
        }
        "###);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2500),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "schedule": "0 0 * * *",
          "delay": 2500,
          "enableNotifications": true
        }
        "###);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2500),
            scripts: Default::default(),
            enable_notifications: false,
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "schedule": "0 0 * * *",
          "delay": 2500,
          "enableNotifications": false
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: None,
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_eq!(
            serde_json::from_str::<WebPageTrackerSettings>(
                &json!({ "revisions": 3, "delay": 2000, "enableNotifications": true }).to_string()
            )?,
            settings
        );

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_eq!(
            serde_json::from_str::<WebPageTrackerSettings>(
                &json!({ "revisions": 3, "delay": 2000, "schedule": "0 0 * * *", "enableNotifications": true }).to_string()
            )?,
            settings
        );

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2000),
            scripts: Some(
                [(
                    WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                    "return resource;".to_string(),
                )]
                .into_iter()
                .collect(),
            ),
            enable_notifications: true,
        };
        assert_eq!(
            serde_json::from_str::<WebPageTrackerSettings>(
                &json!({
                    "revisions": 3,
                    "delay": 2000,
                    "schedule": "0 0 * * *",
                    "scripts": { "resourceFilterMap": "return resource;" },
                    "enableNotifications": true
                })
                .to_string()
            )?,
            settings
        );

        let settings = WebPageTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2000),
            scripts: Some(
                [(
                    WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                    "return resource;".to_string(),
                )]
                .into_iter()
                .collect(),
            ),
            enable_notifications: false,
        };
        assert_eq!(
            serde_json::from_str::<WebPageTrackerSettings>(
                &json!({
                    "revisions": 3,
                    "delay": 2000,
                    "schedule": "0 0 * * *",
                    "scripts": { "resourceFilterMap": "return resource;" },
                    "enableNotifications": false
                })
                .to_string()
            )?,
            settings
        );

        Ok(())
    }
}
