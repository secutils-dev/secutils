use serde::{Deserialize, Serialize};

/// Represents error response if scraper couldn't extract content.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperContentError {
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::WebScraperContentError;
    use insta::assert_json_snapshot;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScraperContentError>(
                r#"
{
    "message": "some-error"
}
          "#
            )?,
            WebScraperContentError {
                message: "some-error".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperContentError {
            message: "some-error".to_string(),
        }, @r###"
        {
          "message": "some-error"
        }
        "###);

        Ok(())
    }
}
