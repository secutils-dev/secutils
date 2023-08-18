use crate::utils::{WebPageResource, WebPageResourceContent};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

/// Represents response with scraped resources.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResource {
    /// The URL resources is loaded from.
    pub url: Option<Url>,
    /// Resource content descriptor (size, data etc.), if available.
    pub content: Option<WebPageResourceContent>,
}

impl From<WebScraperResource> for WebPageResource {
    fn from(value: WebScraperResource) -> Self {
        Self {
            url: value.url,
            content: value.content,
            diff_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WebScraperResource, WebScraperResourcesResponse};
    use crate::utils::{WebPageResourceContent, WebPageResourceContentData};
    use insta::assert_json_snapshot;
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
        { "url": "https://secutils.dev/script.js", "content": { "data": { "type": "sha1", "value": "some-digest" }, "size": 123 } },
        { "content": { "data": { "type": "sha1", "value": "another-digest" }, "size": 321 } }
    ],
    "styles": [
        { "url": "https://secutils.dev/style.css", "content": { "data": { "type": "sha1", "value": "some-css-digest" }, "size": 456 } },
        { "content": { "data": { "type": "sha1", "value": "another-css-digest" }, "size": 654 } }
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
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                            size: 123,
                        })
                    },
                    WebScraperResource {
                        url: None,
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                            size: 321,
                        })
                    }
                ],
                styles: vec![
                    WebScraperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1("some-css-digest".to_string()),
                            size: 456,
                        })
                    },
                    WebScraperResource {
                        url: None,
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1(
                                "another-css-digest".to_string()
                            ),
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
        { "content": { "data": { "type": "sha1", "value": "another-digest" }, "size": 123 } }
    ],
    "styles": [
        { "url": "https://secutils.dev/style.css" },
        { "content": { "data": { "type": "sha1", "value": "another-css-digest" }, "size": 321 } }
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
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1("another-digest".to_string()),
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
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1(
                                "another-css-digest".to_string()
                            ),
                            size: 321,
                        }),
                    }
                ],
            }
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperResourcesResponse {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![
                WebScraperResource {
                    url: Some(Url::parse("https://secutils.dev/script.js")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123,
                    })
                },
                WebScraperResource {
                    url: None,
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                        size: 321,
                    })
                }
            ],
            styles: vec![
                WebScraperResource {
                    url: Some(Url::parse("https://secutils.dev/style.css")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-css-digest".to_string()),
                        size: 456,
                    })
                },
                WebScraperResource {
                    url: None,
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1(
                            "another-css-digest".to_string()
                        ),
                        size: 654,
                    })
                }
            ],
        }, @r###"
        {
          "timestamp": 946720800,
          "scripts": [
            {
              "url": "https://secutils.dev/script.js",
              "content": {
                "data": {
                  "type": "sha1",
                  "value": "some-digest"
                },
                "size": 123
              }
            },
            {
              "url": null,
              "content": {
                "data": {
                  "type": "sha1",
                  "value": "another-digest"
                },
                "size": 321
              }
            }
          ],
          "styles": [
            {
              "url": "https://secutils.dev/style.css",
              "content": {
                "data": {
                  "type": "sha1",
                  "value": "some-css-digest"
                },
                "size": 456
              }
            },
            {
              "url": null,
              "content": {
                "data": {
                  "type": "sha1",
                  "value": "another-css-digest"
                },
                "size": 654
              }
            }
          ]
        }
        "###);

        Ok(())
    }
}
