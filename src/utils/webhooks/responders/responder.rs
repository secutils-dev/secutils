use crate::utils::{ResponderMethod, ResponderSettings};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Responder {
    /// Unique responder id (UUIDv7).
    pub id: Uuid,
    /// Arbitrary name of the responder.
    pub name: String,
    /// Path of the responder.
    pub path: String,
    /// HTTP method of the responder.
    pub method: ResponderMethod,
    /// Miscellaneous responder settings.
    pub settings: ResponderSettings,
    /// Date and time when the web page tracker was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::utils::{Responder, ResponderMethod, ResponderSettings};
    use insta::assert_json_snapshot;
    use std::time::Duration;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "some-name".to_string(),
            path: "/path".to_string(),
            method: ResponderMethod::Post,
            settings: ResponderSettings {
                requests_to_track: 10,
                status_code: 123,
                body: Some("some-body".to_string()),
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: Duration::from_millis(1000),
            },
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "path": "/path",
          "method": "POST",
          "settings": {
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
          },
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<Responder>(
                r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "path": "/path",
          "method": "POST",
          "settings": {
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
          },
          "createdAt": 946720800
        }
        "###
            )?,
            Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "some-name".to_string(),
                path: "/path".to_string(),
                method: ResponderMethod::Post,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 123,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    delay: Duration::from_millis(1000),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}
