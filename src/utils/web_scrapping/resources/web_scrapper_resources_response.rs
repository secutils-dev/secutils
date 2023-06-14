use serde::Deserialize;
use time::OffsetDateTime;
use url::Url;

/// Represents response with scrapped resources.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScrapperResourcesResponse {
    /// Timestamp indicating when resources were fetched.
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
    /// List of JavaScript resources.
    pub scripts: WebScrapperResourceBundle,
    /// List of CSS resources.
    pub styles: WebScrapperResourceBundle,
}

/// Represents both external and inline resources of a particular type.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScrapperResourceBundle {
    /// List of external resources.
    pub external: Vec<WebScrapperResource>,
    /// List of inline resources.
    pub inline: Vec<WebScrapperResource>,
}

/// Describes either external or inline resource.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScrapperResource {
    /// The URL resources is loaded from.
    pub url: Option<Url>,
    /// SHA256 digest of the external resource content, if available.
    pub digest: Option<String>,
    /// Size of the inline resource content, if available, in bytes.
    pub size: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::{WebScrapperResource, WebScrapperResourceBundle, WebScrapperResourcesResponse};
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScrapperResourcesResponse>(
                r###"
{
    "timestamp": 946720800,
    "scripts": { 
        "external": [{ "url": "https://secutils.dev/script.js", "digest": "some-digest", "size": 123 }],
        "inline": [{ "digest": "another-digest", "size": 321 }]
    },
    "styles": { 
        "external": [{ "url": "https://secutils.dev/style.css", "digest": "some-css-digest", "size": 456 }],
        "inline": [{ "digest": "another-css-digest", "size": 654 }]
    }
}
          "###
            )?,
            WebScrapperResourcesResponse {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                scripts: WebScrapperResourceBundle {
                    external: vec![WebScrapperResource {
                        url: Some(Url::parse("https://secutils.dev/script.js")?),
                        digest: Some("some-digest".to_string()),
                        size: Some(123),
                    }],
                    inline: vec![WebScrapperResource {
                        url: None,
                        digest: Some("another-digest".to_string()),
                        size: Some(321),
                    }]
                },
                styles: WebScrapperResourceBundle {
                    external: vec![WebScrapperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        digest: Some("some-css-digest".to_string()),
                        size: Some(456),
                    }],
                    inline: vec![WebScrapperResource {
                        url: None,
                        digest: Some("another-css-digest".to_string()),
                        size: Some(654),
                    }]
                },
            }
        );

        Ok(())
    }

    #[test]
    fn deserialization_without_optional_values() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScrapperResourcesResponse>(
                r###"
{
    "timestamp": 946720800,
    "scripts": { 
        "external": [{ "url": "https://secutils.dev/script.js" }],
        "inline": [{ "digest": "another-digest" }]
    },
    "styles": { 
        "external": [{ "url": "https://secutils.dev/style.css" }],
        "inline": [{ "digest": "another-css-digest" }]
    }
}
          "###
            )?,
            WebScrapperResourcesResponse {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                scripts: WebScrapperResourceBundle {
                    external: vec![WebScrapperResource {
                        url: Some(Url::parse("https://secutils.dev/script.js")?),
                        digest: None,
                        size: None,
                    }],
                    inline: vec![WebScrapperResource {
                        url: None,
                        digest: Some("another-digest".to_string()),
                        size: None,
                    }]
                },
                styles: WebScrapperResourceBundle {
                    external: vec![WebScrapperResource {
                        url: Some(Url::parse("https://secutils.dev/style.css")?),
                        digest: None,
                        size: None,
                    }],
                    inline: vec![WebScrapperResource {
                        url: None,
                        digest: Some("another-css-digest".to_string()),
                        size: None,
                    }]
                },
            }
        );

        Ok(())
    }
}
