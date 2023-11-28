use serde::{Deserialize, Serialize};
use std::{borrow::Cow, net::IpAddr};
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
    /// IP address of the client that made the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_address: Option<IpAddr>,
    /// HTTP method of the request.
    pub method: Cow<'a, str>,
    /// HTTP headers of the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<ResponderRequestHeaders<'a>>,
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
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("Content-Type"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "clientAddress": "127.0.0.1",
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
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "method": "POST",
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
          "clientAddress": "127.0.0.1",
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
                client_address: Some("127.0.0.1".parse()?),
                method: Cow::Borrowed("GET"),
                headers: Some(vec![(
                    Cow::Borrowed("Content-Type"),
                    Cow::Borrowed(&[1, 2, 3])
                )]),
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
                body: None,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}