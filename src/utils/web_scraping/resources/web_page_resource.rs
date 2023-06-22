use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageResource;
    use insta::assert_json_snapshot;
    use serde_json::json;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResource {
            url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
            digest: Some("some-digest".to_string()),
            size: Some(123)
        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "digest": "some-digest",
          "size": 123
        }
        "###);

        assert_json_snapshot!(WebPageResource {
            url: None,
            digest: Some("some-digest".to_string()),
            size: Some(123)
        }, @r###"
        {
          "digest": "some-digest",
          "size": 123
        }
        "###);

        assert_json_snapshot!(WebPageResource {
            url: None,
            digest: None,
            size: None
        }, @"{}");

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "url": "https://localhost:1234/my/app?q=2", "digest": "some-digest", "size": 123 }).to_string()
            )?,
            WebPageResource {
                url: Some(Url::parse("https://localhost:1234/my/app?q=2")?),
                digest: Some("some-digest".to_string()),
                size: Some(123)
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
                digest: None,
                size: None
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "digest": "some-digest", "size": 123 }).to_string()
            )?,
            WebPageResource {
                url: None,
                digest: Some("some-digest".to_string()),
                size: Some(123)
            }
        );

        Ok(())
    }
}
