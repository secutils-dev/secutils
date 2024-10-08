use crate::utils::webhooks::{ResponderLocation, ResponderMethod, ResponderSettings};
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
    /// Location of the responder.
    pub location: ResponderLocation,
    /// HTTP method of the responder.
    pub method: ResponderMethod,
    /// Indicates whether the responder is enabled.
    pub enabled: bool,
    /// Miscellaneous responder settings.
    pub settings: ResponderSettings,
    /// Date and time when the web page tracker was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    /// Date and time when the web page tracker was last updated.
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::{
        Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "some-name".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/path".to_string(),
                subdomain_prefix: None
            },
            method: ResponderMethod::Post,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 10,
                status_code: 123,
                body: Some("some-body".to_string()),
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                script: Some("return { body: `custom body` };".to_string()),
            },
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "location": {
            "pathType": "=",
            "path": "/path"
          },
          "method": "POST",
          "enabled": true,
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
            "script": "return { body: `custom body` };"
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<Responder>(
                r#"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "location": {
            "path": "/path",
            "pathType": "="
          },
          "method": "POST",
          "enabled": true,
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
            "script": "return { body: `custom body` };"
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "#
            )?,
            Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "some-name".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Post,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 123,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: Some("return { body: `custom body` };".to_string()),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?
            }
        );

        Ok(())
    }
}
