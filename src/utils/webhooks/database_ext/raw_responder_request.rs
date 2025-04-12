use crate::utils::webhooks::{ResponderRequest, ResponderRequestHeaders};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, net::SocketAddr};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResponderRequest {
    pub id: Uuid,
    pub responder_id: Uuid,
    pub data: Vec<u8>,
    pub created_at: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
struct RawResponderRequestData<'a> {
    client_address: Option<SocketAddr>,
    method: Cow<'a, str>,
    headers: Option<ResponderRequestHeaders<'a>>,
    url: Cow<'a, str>,
    body: Option<Cow<'a, [u8]>>,
}

impl TryFrom<RawResponderRequest> for ResponderRequest<'_> {
    type Error = anyhow::Error;

    fn try_from(raw: RawResponderRequest) -> Result<Self, Self::Error> {
        let raw_data = postcard::from_bytes::<RawResponderRequestData>(&raw.data)?;
        Ok(Self {
            id: raw.id,
            responder_id: raw.responder_id,
            client_address: raw_data.client_address,
            method: raw_data.method,
            body: raw_data.body,
            headers: raw_data.headers,
            url: raw_data.url,
            created_at: raw.created_at,
        })
    }
}

impl TryFrom<&ResponderRequest<'_>> for RawResponderRequest {
    type Error = anyhow::Error;

    fn try_from(item: &ResponderRequest) -> Result<Self, Self::Error> {
        let raw_data = RawResponderRequestData {
            client_address: item.client_address,
            method: item.method.clone(),
            body: item.body.clone(),
            headers: item.headers.clone(),
            url: item.url.clone(),
        };

        Ok(Self {
            id: item.id,
            responder_id: item.responder_id,
            data: postcard::to_stdvec(&raw_data)?,
            created_at: item.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawResponderRequest;
    use crate::utils::webhooks::ResponderRequest;
    use std::borrow::Cow;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_raw_responder_request() -> anyhow::Result<()> {
        assert_eq!(
            RawResponderRequest::try_from(&ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: None,
                method: Cow::Owned("post".to_string()),
                headers: None,
                body: None,
                url: Cow::Borrowed("/some-path?query=value"),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            })?,
            RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: vec![
                    0, 4, 112, 111, 115, 116, 0, 22, 47, 115, 111, 109, 101, 45, 112, 97, 116, 104,
                    63, 113, 117, 101, 114, 121, 61, 118, 97, 108, 117, 101, 0
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        assert_eq!(
            RawResponderRequest::try_from(&ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: Some("127.0.0.1:8080".parse()?),
                method: Cow::Owned("post".to_string()),
                headers: Some(vec![(
                    Cow::Owned("Content-Type".to_string()),
                    Cow::Owned(vec![1, 2, 3]),
                )]),
                url: Cow::Borrowed("/some-path?query=value"),
                body: Some(Cow::Owned(vec![4, 5, 6])),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            })?,
            RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: vec![
                    1, 0, 127, 0, 0, 1, 144, 63, 4, 112, 111, 115, 116, 1, 1, 12, 67, 111, 110,
                    116, 101, 110, 116, 45, 84, 121, 112, 101, 3, 1, 2, 3, 22, 47, 115, 111, 109,
                    101, 45, 112, 97, 116, 104, 63, 113, 117, 101, 114, 121, 61, 118, 97, 108, 117,
                    101, 1, 3, 4, 5, 6
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_responder_request() -> anyhow::Result<()> {
        assert_eq!(
            ResponderRequest::try_from(RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: vec![
                    0, 4, 112, 111, 115, 116, 0, 22, 47, 115, 111, 109, 101, 45, 112, 97, 116, 104,
                    63, 113, 117, 101, 114, 121, 61, 118, 97, 108, 117, 101, 0
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: None,
                method: Cow::Owned("post".to_string()),
                headers: None,
                body: None,
                url: Cow::Borrowed("/some-path?query=value"),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        assert_eq!(
            ResponderRequest::try_from(RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: vec![
                    1, 0, 127, 0, 0, 1, 144, 63, 4, 112, 111, 115, 116, 1, 1, 12, 67, 111, 110,
                    116, 101, 110, 116, 45, 84, 121, 112, 101, 3, 1, 2, 3, 22, 47, 115, 111, 109,
                    101, 45, 112, 97, 116, 104, 63, 113, 117, 101, 114, 121, 61, 118, 97, 108, 117,
                    101, 1, 3, 4, 5, 6
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: Some("127.0.0.1:8080".parse()?),
                method: Cow::Owned("post".to_string()),
                headers: Some(vec![(
                    Cow::Owned("Content-Type".to_string()),
                    Cow::Owned(vec![1, 2, 3]),
                )]),
                body: Some(Cow::Owned(vec![4, 5, 6])),
                url: Cow::Borrowed("/some-path?query=value"),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}
