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
}

#[cfg(test)]
mod tests {
    use crate::utils::ResponderRequest;
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
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?
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
          "createdAt": 946720800
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
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?
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
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
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
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}
