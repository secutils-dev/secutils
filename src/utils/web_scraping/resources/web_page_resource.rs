use crate::utils::WebPageResourceContent;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<WebPageResourceContent>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{WebPageResource, WebPageResourceContent};
    use insta::assert_json_snapshot;
    use serde_json::json;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResource {
            url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
            content: Some(WebPageResourceContent { digest:"some-digest".to_string(), size: 123 })

        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "content": {
            "digest": "some-digest",
            "size": 123
          }
        }
        "###);

        assert_json_snapshot!(WebPageResource {
            url: None,
            content: Some(WebPageResourceContent { digest:"some-digest".to_string(), size: 123 })
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
                content: Some(WebPageResourceContent { digest:"some-digest".to_string(), size: 123 })
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
                content: None
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
                })
            }
        );

        Ok(())
    }
}
