use crate::utils::{WebPageResource, WebPageResourceContent};
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
    /// Resource content descriptor (size, digest etc.), if available.
    pub content: Option<WebScraperResourceContent>,
}

impl From<WebScraperResource> for WebPageResource {
    fn from(value: WebScraperResource) -> Self {
        Self {
            url: value.url,
            content: value.content.map(Into::into),
            diff_status: None,
        }
    }
}

/// Describes resource content.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResourceContent {
    /// SHA-1 digest of the external resource content.
    pub digest: String,
    /// Size of the inline resource content, in bytes.
    pub size: usize,
}

impl From<WebScraperResourceContent> for WebPageResourceContent {
    fn from(value: WebScraperResourceContent) -> Self {
        Self {
            digest: value.digest,
            size: value.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WebScraperResource, WebScraperResourceContent, WebScraperResourcesResponse};
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
        { "url": "https://secutils.dev/script.js", "content": { "digest": "some-digest", "size": 123 } },
        { "content": { "digest": "another-digest", "size": 321 } }
    ],
    "styles": [
        { "url": "https://secutils.dev/style.css", "content": { "digest": "some-css-digest", "size": 456 } },
        { "content": { "digest": "another-css-digest", "size": 654 } }
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
                        content: Some(WebScraperResourceContent {
                            digest: "some-digest".to_string(),
                            size: 123,
                        })
                    },
                    WebScraperResource {
                        url: None,
                        content: Some(WebScraperResourceContent {
                            digest: "another-digest".to_string(),
                            size: 321,
                        })
                    }
                ],
                styles: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        content: Some(WebScraperResourceContent {
                            digest: "some-css-digest".to_string(),
                            size: 456,
                        })
                    },
                    WebScraperResource {
                        url: None,
                        content: Some(WebScraperResourceContent {
                            digest: "another-css-digest".to_string(),
                            size: 654,
                        })
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
        { "content": { "digest": "another-digest", "size": 123 } }
    ],
    "styles": [
        { "url": "https://secutils.dev/style.css" },
        { "content": { "digest": "another-css-digest", "size": 321 } }
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
                        content: None
                    },
                    WebScraperResource {
                        url: None,
                        content: Some(WebScraperResourceContent {
                            digest: "another-digest".to_string(),
                            size: 123,
                        })
                    }
                ],
                styles: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        content: None
                    },
                    WebScraperResource {
                        url: None,
                        content: Some(WebScraperResourceContent {
                            digest: "another-css-digest".to_string(),
                            size: 321,
                        }),
                    }
                ],
            }
        );

        Ok(())
    }
}
