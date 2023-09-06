use crate::utils::{WebPageResourcesRevision, WebPageResourcesTracker};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrapingActionResult {
    #[serde(rename_all = "camelCase")]
    FetchWebPageResources {
        tracker_name: String,
        revisions: Vec<WebPageResourcesRevision>,
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
    use crate::{
        tests::MockWebPageResourcesTrackerBuilder,
        utils::{
            UtilsWebScrapingActionResult, WebPageResource, WebPageResourceContent,
            WebPageResourceContentData, WebPageResourcesRevision,
        },
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let web_page_resources = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/script.js")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 123,
                }),
                diff_status: None,
            }],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/style.css?fonts=2")?),
                content: None,
                diff_status: None,
            }],
        };

        assert_json_snapshot!(UtilsWebScrapingActionResult::FetchWebPageResources {
            tracker_name: "tracker".to_string(),
            revisions: vec![web_page_resources]
        }, @r###"
        {
          "type": "fetchWebPageResources",
          "value": {
            "trackerName": "tracker",
            "revisions": [
              {
                "timestamp": 946720800,
                "scripts": [
                  {
                    "url": "http://localhost:1234/script.js",
                    "content": {
                      "data": {
                        "type": "sha1",
                        "value": "some-digest"
                      },
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

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_schedule("0 0 0 1 * *")
        .build();
        assert_json_snapshot!(UtilsWebScrapingActionResult::SaveWebPageResourcesTracker {
            tracker
        }, @r###"
        {
          "type": "saveWebPageResourcesTracker",
          "value": {
            "tracker": {
              "name": "some-name",
              "url": "http://localhost:1234/my/app?q=2",
              "revisions": 3,
              "delay": 2000,
              "schedule": "0 0 0 1 * *"
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
