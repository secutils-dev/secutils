use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::time::Duration;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderSettings {
    /// Number of requests to track.
    #[serde(default)]
    pub requests_to_track: usize,
    /// HTTP status code to respond with.
    pub status_code: u16,
    /// Optional body to respond with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Optional headers to respond with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,
    /// Number of milliseconds to wait before responding to request.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub delay: Duration,
}

#[cfg(test)]
mod tests {
    use crate::utils::ResponderSettings;
    use insta::assert_json_snapshot;
    use std::time::Duration;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderSettings {
            requests_to_track: 10,
            status_code: 123,
            body: Some("some-body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: Duration::from_millis(1000),
        }, @r###"
        {
          "requestsToTrack": 10,
          "statusCode": 123,
          "body": "some-body",
          "headers": [
            [
              "key",
              "value"
            ]
          ],
          "delay": 1000
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderSettings>(
                r#"
        {
          "requestsToTrack": 10,
          "statusCode": 123,
          "body": "some-body",
          "headers": [
            [
              "key",
              "value"
            ]
          ],
          "delay": 1000
        }
        "#
            )?,
            ResponderSettings {
                requests_to_track: 10,
                status_code: 123,
                body: Some("some-body".to_string()),
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: Duration::from_millis(1000),
            }
        );

        assert_eq!(
            serde_json::from_str::<ResponderSettings>(
                r#"
        {
          "statusCode": 123,
          "delay": 1000
        }
        "#
            )?,
            ResponderSettings {
                requests_to_track: 0,
                status_code: 123,
                body: None,
                headers: None,
                delay: Duration::from_millis(1000),
            }
        );

        Ok(())
    }
}
