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
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageResourcesTracker;
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::time::Duration;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourcesTracker {
            name: "some-name".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2500),
            schedule: None,
        }, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3,
          "delay": 2500
        }
        "###);

        assert_json_snapshot!(WebPageResourcesTracker {
            name: "some-name".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2500),
            schedule: Some("0 0 * * *".to_string()),
        }, @r###"
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
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({ "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000 })
                    .to_string()
            )?,
            WebPageResourcesTracker {
                name: "some-name".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3,
                delay: Duration::from_millis(2000),
                schedule: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({ "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000, "schedule": "0 0 * * *" })
                    .to_string()
            )?,
            WebPageResourcesTracker {
                name: "some-name".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3,
                delay: Duration::from_millis(2000),
                schedule: Some("0 0 * * *".to_string()),
            }
        );

        Ok(())
    }
}
