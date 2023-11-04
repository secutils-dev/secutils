use crate::utils::WebPageResourceContentData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourceContent {
    /// Resource content data.
    pub data: WebPageResourceContentData,
    /// Size of the inline resource content, in bytes.
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use crate::utils::{WebPageResourceContent, WebPageResourceContentData};
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourceContent {
            data: WebPageResourceContentData::Sha1("some-digest".to_string()),
            size: 123
        }, @r###"
        {
          "data": {
            "sha1": "some-digest"
          },
          "size": 123
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourceContent>(
                &json!({ "data": { "sha1": "some-digest" }, "size": 123 }).to_string()
            )?,
            WebPageResourceContent {
                data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                size: 123
            }
        );

        Ok(())
    }
}
