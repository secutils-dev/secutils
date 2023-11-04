use crate::utils::WebPageResource;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResourcesRevision {
    /// Unique private key id (UUIDv7).
    pub id: Uuid,
    /// Id of the user who owns the tracker.
    #[serde(skip_serializing)]
    pub tracker_id: Uuid,
    /// List of JavaScript resources.
    pub scripts: Vec<WebPageResource>,
    /// List of CSS resources.
    pub styles: Vec<WebPageResource>,
    /// Timestamp indicating when resources were fetched.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

impl WebPageResourcesRevision {
    /// Returns `true` if any of the scripts or styles has a diff status, otherwise returns `false`.
    pub fn has_diff(&self) -> bool {
        self.scripts
            .iter()
            .chain(self.styles.iter())
            .any(|resource| resource.diff_status.is_some())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        web_scraping::WebPageResourceDiffStatus, WebPageResource, WebPageResourceContent,
        WebPageResourceContentData, WebPageResourcesRevision,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourcesRevision {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
            created_at: OffsetDateTime::from_unix_timestamp(
                946720800,
            )?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: Some(WebPageResourceContent{
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 123
                }),
                diff_status: None,
            }],
                styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                content: Some(WebPageResourceContent{
                    data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                    size: 321
                }),
                diff_status: None,
            }]
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "scripts": [
            {
              "url": "http://localhost:1234/my/app?q=2",
              "content": {
                "data": {
                  "sha1": "some-digest"
                },
                "size": 123
              }
            }
          ],
          "styles": [
            {
              "url": "http://localhost:1234/my/app.css?q=2",
              "content": {
                "data": {
                  "sha1": "another-digest"
                },
                "size": 321
              }
            }
          ],
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }

    #[test]
    fn checks_has_diff() -> anyhow::Result<()> {
        let revision = WebPageResourcesRevision {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 123,
                }),
                diff_status: None,
            }],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                    size: 321,
                }),
                diff_status: None,
            }],
        };
        assert!(!revision.has_diff());

        let revision = WebPageResourcesRevision {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123,
                    }),
                    diff_status: None,
                },
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123,
                    }),
                    diff_status: Some(WebPageResourceDiffStatus::Added),
                },
            ],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                    size: 321,
                }),
                diff_status: None,
            }],
        };
        assert!(revision.has_diff());

        let revision = WebPageResourcesRevision {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123,
                    }),
                    diff_status: None,
                },
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123,
                    }),
                    diff_status: None,
                },
            ],
            styles: vec![
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                        size: 321,
                    }),
                    diff_status: None,
                },
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                        size: 321,
                    }),
                    diff_status: Some(WebPageResourceDiffStatus::Removed),
                },
            ],
        };
        assert!(revision.has_diff());

        Ok(())
    }
}
