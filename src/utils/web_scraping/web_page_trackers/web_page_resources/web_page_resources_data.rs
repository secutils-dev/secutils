use crate::utils::WebPageResource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourcesData<
    R: Clone + Serialize + for<'des> Deserialize<'des> = WebPageResource,
> {
    /// List of JavaScript resources.
    #[serde(bound(deserialize = ""))]
    pub scripts: Vec<R>,
    /// List of CSS resources.
    #[serde(bound(deserialize = ""))]
    pub styles: Vec<R>,
}

impl WebPageResourcesData {
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
        WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        WebPageResourceDiffStatus, WebPageResourcesData,
    };
    use insta::assert_json_snapshot;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourcesData {
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
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            WebPageResourcesData {
                scripts: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123
                    }),
                    diff_status: None,
                }],
                styles: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                        size: 321
                    }),
                    diff_status: None,
                }]
            },
            serde_json::from_str(
                r#"
        {
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
          ]
        }
        "#
            )?
        );

        Ok(())
    }

    #[test]
    fn checks_has_diff() -> anyhow::Result<()> {
        let data = WebPageResourcesData {
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
        assert!(!data.has_diff());

        let data = WebPageResourcesData {
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
        assert!(data.has_diff());

        let data = WebPageResourcesData {
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
        assert!(data.has_diff());

        Ok(())
    }
}
