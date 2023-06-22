use serde::Deserialize;
use time::OffsetDateTime;
use url::Url;

/// Represents response with scraped resources.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResourcesResponse {
    /// Timestamp indicating when resources were fetched.
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
    /// List of JavaScript resources.
    pub scripts: Vec<WebScraperResource>,
    /// List of CSS resources.
    pub styles: Vec<WebScraperResource>,
}

/// Describes either external or inline resource.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResource {
    /// The URL resources is loaded from.
    pub url: Option<Url>,
    /// SHA256 digest of the external resource content, if available.
    pub digest: Option<String>,
    /// Size of the inline resource content, if available, in bytes.
    pub size: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::{WebScraperResource, WebScraperResourcesResponse};
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScraperResourcesResponse>(
                r###"
{
    "timestamp": 946720800,
    "scripts": [
        { "url": "https://secutils.dev/script.js", "digest": "some-digest", "size": 123 },
        { "digest": "another-digest", "size": 321 }
    ],
    "styles": [
        { "url": "https://secutils.dev/style.css", "digest": "some-css-digest", "size": 456 },
        { "digest": "another-css-digest", "size": 654 }
    ]
}
          "###
            )?,
            WebScraperResourcesResponse {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                scripts: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/script.js")?),
                        digest: Some("some-digest".to_string()),
                        size: Some(123),
                    },
                    WebScraperResource {
                        url: None,
                        digest: Some("another-digest".to_string()),
                        size: Some(321),
                    }
                ],
                styles: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        digest: Some("some-css-digest".to_string()),
                        size: Some(456),
                    },
                    WebScraperResource {
                        url: None,
                        digest: Some("another-css-digest".to_string()),
                        size: Some(654),
                    }
                ],
            }
        );

        Ok(())
    }

    #[test]
    fn deserialization_without_optional_values() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScraperResourcesResponse>(
                r###"
{
    "timestamp": 946720800,
    "scripts": [
        { "url": "https://secutils.dev/script.js" },
        { "digest": "another-digest" }
    ],
    "styles": [
        { "url": "https://secutils.dev/style.css" },
        { "digest": "another-css-digest" }
    ]
}
          "###
            )?,
            WebScraperResourcesResponse {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                scripts: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/script.js")?),
                        digest: None,
                        size: None,
                    },
                    WebScraperResource {
                        url: None,
                        digest: Some("another-digest".to_string()),
                        size: None,
                    }
                ],
                styles: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        digest: None,
                        size: None,
                    },
                    WebScraperResource {
                        url: None,
                        digest: Some("another-css-digest".to_string()),
                        size: None,
                    }
                ],
            }
        );

        Ok(())
    }
}
