use crate::utils::WebPageResource;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourcesRevision {
    /// Timestamp indicating when resources were fetched.
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
    /// List of JavaScript resources.
    pub scripts: Vec<WebPageResource>,
    /// List of CSS resources.
    pub styles: Vec<WebPageResource>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{WebPageResource, WebPageResourceContent, WebPageResourcesRevision};
    use insta::assert_json_snapshot;
    use serde_json::json;
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(
                946720800,
            )?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: Some(WebPageResourceContent{
                    digest: "some-digest".to_string(),
                    size: 123
                }),
                diff_status: None,
            }],
                styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                content: Some(WebPageResourceContent{
                    digest: "another-digest".to_string(),
                    size: 321
                }),
                diff_status: None,
            }]
        }, @r###"
        {
          "timestamp": 946720800,
          "scripts": [
            {
              "url": "http://localhost:1234/my/app?q=2",
              "content": {
                "digest": "some-digest",
                "size": 123
              }
            }
          ],
          "styles": [
            {
              "url": "http://localhost:1234/my/app.css?q=2",
              "content": {
                "digest": "another-digest",
                "size": 321
              }
            }
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourcesRevision>(
                &json!({
                    "timestamp": 946720800,
                    "scripts": [{ "url": "http://localhost:1234/my/app?q=2" }],
                    "styles": [{ "url": "http://localhost:1234/my/app.css?q=2" }]
                })
                .to_string()
            )?,
            WebPageResourcesRevision {
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                scripts: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: None,
                    diff_status: None,
                }],
                styles: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                    content: None,
                    diff_status: None,
                }]
            }
        );

        Ok(())
    }
}
