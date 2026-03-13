use serde::{Deserialize, Serialize};
use std::{borrow::Cow, net::SocketAddr};
use time::OffsetDateTime;
use uuid::Uuid;

pub type ResponderRequestHeaders<'a> = Vec<(Cow<'a, str>, Cow<'a, [u8]>)>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderRequest<'a> {
    /// Unique responder request ID (UUIDv7).
    pub id: Uuid,
    /// Id of the responder captured request belongs to.
    #[serde(skip_serializing)]
    pub responder_id: Uuid,
    /// An internet socket address of the client that made the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_address: Option<SocketAddr>,
    /// HTTP method of the request.
    pub method: Cow<'a, str>,
    /// HTTP headers of the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<ResponderRequestHeaders<'a>>,
    /// HTTP path of the request + query string.
    pub url: Cow<'a, str>,
    /// HTTP body of the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Cow<'a, [u8]>>,
    /// Date and time when the request was captured.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    /// Total server-side processing time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    /// HTTP status code of the response (when response tracking is enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_status_code: Option<u16>,
    /// HTTP headers of the response (when response tracking is enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<ResponderRequestHeaders<'a>>,
    /// HTTP body of the response (when response tracking is enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<Cow<'a, [u8]>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::ResponderRequest;
    use insta::assert_json_snapshot;
    use std::borrow::Cow;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderRequest {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
            client_address: Some("127.0.0.1:8080".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("Content-Type"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
            url: Cow::Borrowed("/some-path?query=value"),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            duration_ms: Some(42),
            response_status_code: Some(200),
            response_headers: Some(vec![(Cow::Borrowed("X-Resp"), Cow::Borrowed(&[7, 8]))]),
            response_body: Some(Cow::Borrowed(&[9, 10])),
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "clientAddress": "127.0.0.1:8080",
          "method": "GET",
          "headers": [
            [
              "Content-Type",
              [
                1,
                2,
                3
              ]
            ]
          ],
          "url": "/some-path?query=value",
          "body": [
            4,
            5,
            6
          ],
          "createdAt": 946720800,
          "durationMs": 42,
          "responseStatusCode": 200,
          "responseHeaders": [
            [
              "X-Resp",
              [
                7,
                8
              ]
            ]
          ],
          "responseBody": [
            9,
            10
          ]
        }
        "###);

        assert_json_snapshot!(ResponderRequest {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
            client_address: None,
            method: Cow::Borrowed("POST"),
            headers: None,
            body: None,
            url: Cow::Borrowed("/some-path?query=value"),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            duration_ms: None,
            response_status_code: None,
            response_headers: None,
            response_body: None,
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "method": "POST",
          "url": "/some-path?query=value",
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderRequest>(
                r#"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "responderId": "00000000-0000-0000-0000-000000000002",
          "clientAddress": "127.0.0.1:8080",
          "method": "GET",
          "headers": [
            [
              "Content-Type",
              [
                1,
                2,
                3
              ]
            ]
          ],
          "url": "/some-path?query=value",
          "body": [
            4,
            5,
            6
          ],
          "createdAt": 946720800
        }
        "#
            )?,
            ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: Some("127.0.0.1:8080".parse()?),
                method: Cow::Borrowed("GET"),
                headers: Some(vec![(
                    Cow::Borrowed("Content-Type"),
                    Cow::Borrowed(&[1, 2, 3])
                )]),
                url: Cow::Borrowed("/some-path?query=value"),
                body: Some(Cow::Borrowed(&[4, 5, 6])),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                duration_ms: None,
                response_status_code: None,
                response_headers: None,
                response_body: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ResponderRequest>(
                r#"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "responderId": "00000000-0000-0000-0000-000000000002",
          "method": "POST",
          "url": "/some-path?query=value",
          "createdAt": 946720800
        }
        "#
            )?,
            ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: None,
                method: Cow::Borrowed("POST"),
                headers: None,
                url: Cow::Borrowed("/some-path?query=value"),
                body: None,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                duration_ms: None,
                response_status_code: None,
                response_headers: None,
                response_body: None,
            }
        );

        Ok(())
    }
}
