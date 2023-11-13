use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Represents response with scraped content.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperContentResponse {
    /// Timestamp indicating when content was fetched.
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
    /// Extracted web page content.
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::WebScraperContentResponse;
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebScraperContentResponse>(
                r#"
{
    "timestamp": 946720800,
    "content": "some-content"
}
          "#
            )?,
            WebScraperContentResponse {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                content: "some-content".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperContentResponse {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            content: "some-content".to_string(),
        }, @r###"
        {
          "timestamp": 946720800,
          "content": "some-content"
        }
        "###);

        Ok(())
    }
}
