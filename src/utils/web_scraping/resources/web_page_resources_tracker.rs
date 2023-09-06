use crate::utils::WebPageResourcesTrackerScripts;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::time::Duration;
use url::Url;

/// We currently support up to 10 revisions of the resources.
pub const MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS: usize = 10;

/// We currently wait up to 60 seconds before starting to track resources.
pub const MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY: Duration = Duration::from_secs(60);

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourcesTracker {
    /// Arbitrary name of the web page resources tracker.
    pub name: String,
    /// URL of the web page to track resources for.
    pub url: Url,
    /// A number of revisions of the resources to track.
    pub revisions: usize,
    /// Number of milliseconds to wait after web page enters "idle" state to start tracking resources.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub delay: Duration,
    /// Optional schedule to track resources on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Optional scripts to inject into the web page before extracting resources to track.
    #[serde(skip_serializing_if = "WebPageResourcesTrackerScripts::is_empty")]
    #[serde(default)]
    pub scripts: WebPageResourcesTrackerScripts,
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockWebPageResourcesTrackerBuilder,
        utils::{web_scraping::resources::WebPageResourcesTrackerScripts, WebPageResourcesTracker},
    };
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3,
          "delay": 2500
        }
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3,
          "delay": 2500,
          "schedule": "0 0 * * *"
        }
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts {
            resource_filter_map: Some("return resource;".to_string()),
        })
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3,
          "delay": 2500,
          "schedule": "0 0 * * *",
          "scripts": {
            "resourceFilterMap": "return resource;"
          }
        }
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts::default())
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3,
          "delay": 2500,
          "schedule": "0 0 * * *"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({ "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000 })
                    .to_string()
            )?,
           tracker
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_schedule("0 0 * * *")
        .build();
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({ "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000, "schedule": "0 0 * * *" })
                    .to_string()
            )?,
            tracker
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts {
            resource_filter_map: Some("return resource;".to_string()),
        })
        .build();
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({
                    "name": "some-name",
                    "url": "http://localhost:1234/my/app?q=2",
                    "revisions": 3,
                    "delay": 2000,
                    "schedule": "0 0 * * *",
                    "scripts": { "resourceFilterMap": "return resource;" }
                })
                .to_string()
            )?,
            tracker
        );

        Ok(())
    }
}
