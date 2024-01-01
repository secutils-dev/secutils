use crate::utils::web_scraping::{WebPageResourceContent, WebPageResourceDiffStatus};
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

    /// Checks if the resource is external (i.e. has URL that's not a data URL or a blob URL).
    pub fn is_external_resource(&self) -> bool {
        self.url
            .as_ref()
            .map(|url| url.scheme() != "data" && url.scheme() != "blob")
            .unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(in crate::utils::web_scraping) struct WebPageResourceInternal {
    pub url: Option<Url>,
    pub content: Option<WebPageResourceContent>,
}

impl From<WebPageResource> for WebPageResourceInternal {
    fn from(resource: WebPageResource) -> Self {
        Self {
            url: resource.url,
            content: resource.content,
        }
    }
}

impl From<WebPageResourceInternal> for WebPageResource {
    fn from(resource: WebPageResourceInternal) -> Self {
        Self {
            url: resource.url,
            content: resource.content,
            diff_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockWebPageResourceBuilder,
        utils::web_scraping::{
            WebPageResource, WebPageResourceContent, WebPageResourceContentData,
            WebPageResourceDiffStatus,
        },
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResource {
            url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
            content: Some(WebPageResourceContent { data: WebPageResourceContentData::Sha1("some-digest".to_string()), size: 123 }),
            diff_status: Some(WebPageResourceDiffStatus::Added),

        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "content": {
            "data": {
              "sha1": "some-digest"
            },
            "size": 123
          },
          "diffStatus": "added"
        }
        "###);

        assert_json_snapshot!(WebPageResource {
            url: None,
            content: Some(WebPageResourceContent { data: WebPageResourceContentData::Sha1("some-digest".to_string()), size: 123 }),
            diff_status: None,
        }, @r###"
        {
          "content": {
            "data": {
              "sha1": "some-digest"
            },
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
                &json!({ "url": "https://localhost:1234/my/app?q=2", "content": { "data": { "sha1": "some-digest" }, "size": 123 } }).to_string()
            )?,
            WebPageResource {
                url: Some(Url::parse("https://localhost:1234/my/app?q=2")?),
                content: Some(WebPageResourceContent { data: WebPageResourceContentData::Sha1("some-digest".to_string()), size: 123 }),
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
                &json!({ "content": { "data": { "sha1": "some-digest" }, "size": 123 } })
                    .to_string()
            )?,
            WebPageResource {
                url: None,
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
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
                data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                size: 123,
            }),
            diff_status: None,
        };
        let resource_with_status = WebPageResource {
            url: Some(Url::parse("http://localhost:1234/one")?),
            content: Some(WebPageResourceContent {
                data: WebPageResourceContentData::Sha1("some-digest".to_string()),
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

    #[test]
    fn correctly_determines_external_resource() -> anyhow::Result<()> {
        for resource in [
            MockWebPageResourceBuilder::with_url(Url::parse("https://localhost:1234/one")?).build(),
            MockWebPageResourceBuilder::with_url(Url::parse("http://127.0.0.1")?).build(),
            MockWebPageResourceBuilder::with_url(Url::parse("file://host/path")?).build(),
        ] {
            assert!(resource.is_external_resource());
        }

        for resource in [
            MockWebPageResourceBuilder::with_content(WebPageResourceContentData::Raw("some-data".to_string()), 123).build(),
            MockWebPageResourceBuilder::with_url(Url::parse(
                "blob:[T16EA0020625914D256FCEFB48581C10163480057470935CDE900D52FC00457F1013E450]",
            )?)
                .build(),
            MockWebPageResourceBuilder::with_url(Url::parse(
                "data:text/css,[T110A02222C3020C0330CB800FA0B2800B8A32088880382FE83C38C02C020E00020238FA]",
            )?)
                .build(),
        ] {
            assert!(!resource.is_external_resource());
        }

        Ok(())
    }
}
