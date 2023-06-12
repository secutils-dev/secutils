use crate::utils::WebPageResource;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrappingActionResult {
    #[serde(rename_all = "camelCase")]
    TrackWebPageResources {
        tracker_name: String,
        resources: Vec<WebPageResource>,
    },
}

#[cfg(test)]
mod tests {
    use crate::utils::{UtilsWebScrappingActionResult, WebPageResource};
    use insta::assert_json_snapshot;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsWebScrappingActionResult::TrackWebPageResources {
            tracker_name: "tracker".to_string(),
            resources: vec![
                WebPageResource { url: Url::parse("http://localhost:1234/script.js")? },
                WebPageResource { url: Url::parse("http://localhost:1234/style.css?fonts=2")? }
            ]
        }, @r###"
        {
          "type": "trackWebPageResources",
          "value": {
            "trackerName": "tracker",
            "resources": [
              {
                "u": "http://localhost:1234/script.js"
              },
              {
                "u": "http://localhost:1234/style.css?fonts=2"
              }
            ]
          }
        }
        "###);

        Ok(())
    }
}
