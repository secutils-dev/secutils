use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::time::Duration;
use url::Url;

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
        }, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3,
          "delay": 2500
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
            }
        );

        Ok(())
    }
}
