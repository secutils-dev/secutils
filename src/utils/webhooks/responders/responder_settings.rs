use crate::users::SecretsAccess;
use serde::{Deserialize, Serialize};

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
    /// Optional JavaScript code to execute for every received request that allows overriding
    /// response status code, body, and headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Controls which user secrets are available to this responder.
    #[serde(default, skip_serializing_if = "SecretsAccess::is_none")]
    pub secrets: SecretsAccess,
}

#[cfg(test)]
mod tests {
    use crate::{users::SecretsAccess, utils::webhooks::ResponderSettings};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderSettings {
            requests_to_track: 10,
            status_code: 123,
            body: Some("some-body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            script: Some("return { body: `custom body` };".to_string()),
            secrets: SecretsAccess::None,
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
          "script": "return { body: `custom body` };"
        }
        "###);

        assert_json_snapshot!(ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
            secrets: SecretsAccess::All,
        }, @r###"
        {
          "requestsToTrack": 0,
          "statusCode": 200,
          "secrets": {
            "type": "all"
          }
        }
        "###);

        assert_json_snapshot!(ResponderSettings {
            requests_to_track: 0,
            status_code: 200,
            body: None,
            headers: None,
            script: None,
            secrets: SecretsAccess::Selected { secrets: vec!["KEY_A".into()] },
        }, @r###"
        {
          "requestsToTrack": 0,
          "statusCode": 200,
          "secrets": {
            "type": "selected",
            "secrets": [
              "KEY_A"
            ]
          }
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
          "script": "return { body: `custom body` };"
        }
        "#
            )?,
            ResponderSettings {
                requests_to_track: 10,
                status_code: 123,
                body: Some("some-body".to_string()),
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                script: Some("return { body: `custom body` };".to_string()),
                secrets: SecretsAccess::None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ResponderSettings>(
                r#"
        {
          "statusCode": 123
        }
        "#
            )?,
            ResponderSettings {
                requests_to_track: 0,
                status_code: 123,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ResponderSettings>(
                r#"{"statusCode":200,"secrets":{"type":"all"}}"#
            )?,
            ResponderSettings {
                requests_to_track: 0,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::All,
            }
        );

        Ok(())
    }
}
