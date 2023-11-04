use crate::utils::WebPageResourcesTrackerScripts;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::time::Duration;

/// We currently support up to 10 revisions of the resources.
pub const MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS: usize = 10;

/// We currently wait up to 60 seconds before starting to track resources.
pub const MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY: Duration = Duration::from_secs(60);

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResourcesTrackerSettings {
    /// A number of revisions of the resources to track.
    pub revisions: usize,
    /// Optional schedule to track resources on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Number of milliseconds to wait after web page enters "idle" state to start tracking resources.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub delay: Duration,
    /// Optional scripts to inject into the web page before extracting resources to track.
    #[serde(skip_serializing_if = "WebPageResourcesTrackerScripts::is_empty")]
    #[serde(default)]
    pub scripts: WebPageResourcesTrackerScripts,
    /// Indicates that resources change notifications are enabled for this tracker.
    pub enable_notifications: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        web_scraping::resources::WebPageResourcesTrackerScripts, WebPageResourcesTrackerSettings,
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let settings = WebPageResourcesTrackerSettings {
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

        let settings = WebPageResourcesTrackerSettings {
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

        let settings = WebPageResourcesTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2500),
            scripts: WebPageResourcesTrackerScripts {
                resource_filter_map: Some("return resource;".to_string()),
            },
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

        let settings = WebPageResourcesTrackerSettings {
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

        let settings = WebPageResourcesTrackerSettings {
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
        let settings = WebPageResourcesTrackerSettings {
            revisions: 3,
            schedule: None,
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerSettings>(
                &json!({ "revisions": 3, "delay": 2000, "enableNotifications": true }).to_string()
            )?,
            settings
        );

        let settings = WebPageResourcesTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            enable_notifications: true,
        };
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerSettings>(
                &json!({ "revisions": 3, "delay": 2000, "schedule": "0 0 * * *", "enableNotifications": true }).to_string()
            )?,
            settings
        );

        let settings = WebPageResourcesTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2000),
            scripts: WebPageResourcesTrackerScripts {
                resource_filter_map: Some("return resource;".to_string()),
            },
            enable_notifications: true,
        };
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerSettings>(
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

        let settings = WebPageResourcesTrackerSettings {
            revisions: 3,
            schedule: Some("0 0 * * *".to_string()),
            delay: Duration::from_millis(2000),
            scripts: WebPageResourcesTrackerScripts {
                resource_filter_map: Some("return resource;".to_string()),
            },
            enable_notifications: false,
        };
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerSettings>(
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
