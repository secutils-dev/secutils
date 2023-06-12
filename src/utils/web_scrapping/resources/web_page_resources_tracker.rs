use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourcesTracker {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "u")]
    pub web_page_url: Url,
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
            web_page_url: Url::parse("http://localhost:1234/my/app?q=2")?
        }, @r###"
        {
          "n": "some-name",
          "u": "http://localhost:1234/my/app?q=2"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTracker>(
                &json!({ "n": "some-name", "u": "http://localhost:1234/my/app?q=2" }).to_string()
            )?,
            WebPageResourcesTracker {
                name: "some-name".to_string(),
                web_page_url: Url::parse("http://localhost:1234/my/app?q=2")?
            }
        );

        Ok(())
    }
}
