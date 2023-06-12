use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResource {
    #[serde(rename = "u")]
    pub url: Url,
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
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
        }, @r###"
        {
          "u": "http://localhost:1234/my/app?q=2"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "u": "https://localhost:1234/my/app?q=2" }).to_string()
            )?,
            WebPageResource {
                url: Url::parse("https://localhost:1234/my/app?q=2")?,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageResource>(
                &json!({ "u": "https://username:password@localhost:1234/my/app?q=2" }).to_string()
            )?,
            WebPageResource {
                url: Url::parse("https://username:password@localhost:1234/my/app?q=2")?,
            }
        );

        Ok(())
    }
}
