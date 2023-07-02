use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebPageResourceContent {
    /// SHA-1 digest of the external resource content.
    pub digest: String,
    /// Size of the inline resource content, in bytes.
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageResourceContent;
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourceContent {
            digest: "some-digest".to_string(),
            size: 123
        }, @r###"
        {
          "digest": "some-digest",
          "size": 123
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourceContent>(
                &json!({ "digest": "some-digest", "size": 123 }).to_string()
            )?,
            WebPageResourceContent {
                digest: "some-digest".to_string(),
                size: 123
            }
        );

        Ok(())
    }
}
