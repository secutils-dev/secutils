use serde::{Deserialize, Serialize};

/// Represents an error returned by the web scraper service.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperErrorResponse {
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::WebScraperErrorResponse;
    use insta::assert_json_snapshot;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScraperErrorResponse>(
                r#"
{
    "message": "some-error"
}
          "#
            )?,
            WebScraperErrorResponse {
                message: "some-error".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperErrorResponse {
            message: "some-error".to_string(),
        }, @r###"
        {
          "message": "some-error"
        }
        "###);

        Ok(())
    }
}
