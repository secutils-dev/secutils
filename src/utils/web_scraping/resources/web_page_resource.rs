use crate::utils::{web_scraping::WebPageResourceDiffStatus, WebPageResourceContent};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<WebPageResourceContent>,
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub diff_status: Option<WebPageResourceDiffStatus>,
}

impl WebPageResource {
    /// Returns the same resource, but with the given diff status.
    pub fn with_diff_status(self, diff_status: WebPageResourceDiffStatus) -> Self {
        Self {
            diff_status: Some(diff_status),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        web_scraping::WebPageResourceDiffStatus, WebPageResource, WebPageResourceContent,
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResource {
            url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
            content: Some(WebPageResourceContent { digest:"some-digest".to_string(), size: 123 }),
            diff_status: Some(WebPageResourceDiffStatus::Added),

        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "content": {
            "digest": "some-digest",
            "size": 123
          },
          "diffStatus": "added"
        }
        "###);

        assert_json_snapshot!(WebPageResource {
            url: None,
            content: Some(WebPageResourceContent { digest:"some-digest".to_string(), size: 123 }),
            diff_status: None,
        }, @r###"
        {
          "content": {
            "digest": "some-digest",
            "size": 123
          }
        }
        "###);

        assert_json_snapshot!(WebPageResource {
            url: None,
            content: None,
            diff_status: None,
        }, @"{}");

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "url": "https://localhost:1234/my/app?q=2", "content": { "digest": "some-digest", "size": 123 } }).to_string()
            )?,
            WebPageResource {
                url: Some(Url::parse("https://localhost:1234/my/app?q=2")?),
                content: Some(WebPageResourceContent { digest:"some-digest".to_string(), size: 123 }),
                diff_status: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "url": "https://username:password@localhost:1234/my/app?q=2" })
                    .to_string()
            )?,
            WebPageResource {
                url: Some(Url::parse(
                    "https://username:password@localhost:1234/my/app?q=2"
                )?),
                content: None,
                diff_status: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "content": { "digest": "some-digest", "size": 123 } }).to_string()
            )?,
            WebPageResource {
                url: None,
                content: Some(WebPageResourceContent {
                    digest: "some-digest".to_string(),
                    size: 123
                }),
                diff_status: None,
            }
        );

        Ok(())
    }

    #[test]
    fn returns_resource_with_diff_status() -> anyhow::Result<()> {
        let resource_without_status = WebPageResource {
            url: Some(Url::parse("http://localhost:1234/one")?),
            content: Some(WebPageResourceContent {
                digest: "some-digest".to_string(),
                size: 123,
            }),
            diff_status: None,
        };
        let resource_with_status = WebPageResource {
            url: Some(Url::parse("http://localhost:1234/one")?),
            content: Some(WebPageResourceContent {
                digest: "some-digest".to_string(),
                size: 123,
            }),
            diff_status: Some(WebPageResourceDiffStatus::Added),
        };

        assert_eq!(
            resource_without_status
                .clone()
                .with_diff_status(WebPageResourceDiffStatus::Added),
            WebPageResource {
                diff_status: Some(WebPageResourceDiffStatus::Added),
                ..resource_without_status.clone()
            }
        );
        assert_eq!(
            resource_without_status
                .clone()
                .with_diff_status(WebPageResourceDiffStatus::Removed),
            WebPageResource {
                diff_status: Some(WebPageResourceDiffStatus::Removed),
                ..resource_without_status.clone()
            }
        );
        assert_eq!(
            resource_without_status
                .clone()
                .with_diff_status(WebPageResourceDiffStatus::Changed),
            WebPageResource {
                diff_status: Some(WebPageResourceDiffStatus::Changed),
                ..resource_without_status
            }
        );

        assert_eq!(
            resource_with_status
                .clone()
                .with_diff_status(WebPageResourceDiffStatus::Added),
            WebPageResource {
                diff_status: Some(WebPageResourceDiffStatus::Added),
                ..resource_with_status.clone()
            }
        );
        assert_eq!(
            resource_with_status
                .clone()
                .with_diff_status(WebPageResourceDiffStatus::Removed),
            WebPageResource {
                diff_status: Some(WebPageResourceDiffStatus::Removed),
                ..resource_with_status.clone()
            }
        );
        assert_eq!(
            resource_with_status
                .clone()
                .with_diff_status(WebPageResourceDiffStatus::Changed),
            WebPageResource {
                diff_status: Some(WebPageResourceDiffStatus::Changed),
                ..resource_with_status
            }
        );

        Ok(())
    }
}
