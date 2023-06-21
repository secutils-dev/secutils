use crate::utils::WebPageResource;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResources {
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
    use crate::utils::{WebPageResource, WebPageResources};
    use insta::assert_json_snapshot;
    use serde_json::json;
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResources {
            timestamp: OffsetDateTime::from_unix_timestamp(
                946720800,
            )?,
            scripts: vec![WebPageResource {
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            digest: Some("some-digest".to_string()),
            size: Some(123)
        }],
            styles: vec![WebPageResource {
            url: Url::parse("http://localhost:1234/my/app.css?q=2")?,
            digest: Some("another-digest".to_string()),
            size: Some(321)
        }]
        }, @r###"
        {
          "timestamp": 946720800,
          "scripts": [
            {
              "url": "http://localhost:1234/my/app?q=2",
              "digest": "some-digest",
              "size": 123
            }
          ],
          "styles": [
            {
              "url": "http://localhost:1234/my/app.css?q=2",
              "digest": "another-digest",
              "size": 321
            }
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResources>(
                &json!({
                    "timestamp": 946720800,
                    "scripts": [{ "url": "http://localhost:1234/my/app?q=2" }],
                    "styles": [{ "url": "http://localhost:1234/my/app.css?q=2" }]
                })
                .to_string()
            )?,
            WebPageResources {
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                scripts: vec![WebPageResource {
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    digest: None,
                    size: None
                }],
                styles: vec![WebPageResource {
                    url: Url::parse("http://localhost:1234/my/app.css?q=2")?,
                    digest: None,
                    size: None
                }]
            }
        );

        Ok(())
    }
}