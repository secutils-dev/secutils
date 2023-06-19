use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourcesTracker {
    /// Arbitrary name of the web page resources tracker.
    pub name: String,
    /// URL of the web page to track resources for.
    pub url: Url,
    /// A number of revisions of the resources to track.
    pub revisions: usize,
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageResourcesTracker;
    use insta::assert_json_snapshot;
    use serde_json::json;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourcesTracker {
            name: "some-name".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3
        }, @r###"
        {
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "revisions": 3
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({ "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3 })
                    .to_string()
            )?,
            WebPageResourcesTracker {
                name: "some-name".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3
            }
        );

        Ok(())
    }
}
