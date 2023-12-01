use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::{collections::HashMap, time::Duration};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageTrackerSettings {
    /// A number of revisions of the web page content to track.
    pub revisions: usize,
    /// Number of milliseconds to wait after web page enters "idle" state to start tracking.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub delay: Duration,
    /// Optional scripts to inject into the tracked web page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<HashMap<String, String>>,
    /// Optional list of HTTP headers that should be sent with the tracker requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
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
            delay: Duration::from_millis(2500),
            scripts: Default::default(),
            headers: Default::default(),
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "delay": 2500
        }
        "###);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            delay: Duration::from_millis(2500),
            scripts: Some(
                [(
                    WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                    "return resource;".to_string(),
                )]
                .into_iter()
                .collect(),
            ),
            headers: Some(
                [("cookie".to_string(), "my-cookie".to_string())]
                    .into_iter()
                    .collect(),
            ),
        };
        assert_json_snapshot!(settings, @r###"
        {
          "revisions": 3,
          "delay": 2500,
          "scripts": {
            "resourceFilterMap": "return resource;"
          },
          "headers": {
            "cookie": "my-cookie"
          }
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let settings = WebPageTrackerSettings {
            revisions: 3,
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            headers: Default::default(),
        };
        assert_eq!(
            serde_json::from_str::<WebPageTrackerSettings>(
                &json!({ "revisions": 3, "delay": 2000 }).to_string()
            )?,
            settings
        );

        let settings = WebPageTrackerSettings {
            revisions: 3,
            delay: Duration::from_millis(2000),
            scripts: Some(
                [(
                    WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                    "return resource;".to_string(),
                )]
                .into_iter()
                .collect(),
            ),
            headers: Some(
                [("cookie".to_string(), "my-cookie".to_string())]
                    .into_iter()
                    .collect(),
            ),
        };
        assert_eq!(
            serde_json::from_str::<WebPageTrackerSettings>(
                &json!({
                    "revisions": 3,
                    "delay": 2000,
                    "scripts": { "resourceFilterMap": "return resource;" },
                    "headers": { "cookie": "my-cookie" }
                })
                .to_string()
            )?,
            settings
        );

        Ok(())
    }
}
