use crate::utils::{WebPageResources, WebPageResourcesTracker};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrapingActionResult {
    #[serde(rename_all = "camelCase")]
    FetchWebPageResources {
        tracker_name: String,
        resources: Vec<WebPageResources>,
    },
    RemoveWebPageResources,
    #[serde(rename_all = "camelCase")]
    SaveWebPageResourcesTracker {
        tracker: WebPageResourcesTracker,
    },
    RemoveWebPageResourcesTracker,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        UtilsWebScrapingActionResult, WebPageResource, WebPageResourceContent, WebPageResources,
        WebPageResourcesTracker,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let web_page_resources = WebPageResources {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/script.js")?),
                content: Some(WebPageResourceContent {
                    digest: "some-digest".to_string(),
                    size: 123,
                }),
            }],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/style.css?fonts=2")?),
                content: None,
            }],
        };

        assert_json_snapshot!(UtilsWebScrapingActionResult::FetchWebPageResources {
            tracker_name: "tracker".to_string(),
            resources: vec![web_page_resources]
        }, @r###"
        {
          "type": "fetchWebPageResources",
          "value": {
            "trackerName": "tracker",
            "resources": [
              {
                "timestamp": 946720800,
                "scripts": [
                  {
                    "url": "http://localhost:1234/script.js",
                    "content": {
                      "digest": "some-digest",
                      "size": 123
                    }
                  }
                ],
                "styles": [
                  {
                    "url": "http://localhost:1234/style.css?fonts=2"
                  }
                ]
              }
            ]
          }
        }
        "###);

        assert_json_snapshot!(UtilsWebScrapingActionResult::RemoveWebPageResources, @r###"
        {
          "type": "removeWebPageResources"
        }
        "###);

        assert_json_snapshot!(UtilsWebScrapingActionResult::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "some-name".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3
            }
        }, @r###"
        {
          "type": "saveWebPageResourcesTracker",
          "value": {
            "tracker": {
              "name": "some-name",
              "url": "http://localhost:1234/my/app?q=2",
              "revisions": 3
            }
          }
        }
        "###);

        assert_json_snapshot!(UtilsWebScrapingActionResult::RemoveWebPageResourcesTracker, @r###"
        {
          "type": "removeWebPageResourcesTracker"
        }
        "###);

        Ok(())
    }
}
