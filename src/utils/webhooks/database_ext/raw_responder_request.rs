use crate::utils::{webhooks::ResponderRequestHeaders, ResponderRequest};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, net::IpAddr};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResponderRequest {
    pub id: Vec<u8>,
    pub responder_id: Vec<u8>,
    pub data: Vec<u8>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
struct RawResponderRequestData<'a> {
    client_address: Option<IpAddr>,
    method: Cow<'a, str>,
    headers: Option<ResponderRequestHeaders<'a>>,
    body: Option<Cow<'a, [u8]>>,
}

impl<'a> TryFrom<RawResponderRequest> for ResponderRequest<'a> {
    type Error = anyhow::Error;

    fn try_from(raw: RawResponderRequest) -> Result<Self, Self::Error> {
        let raw_data = postcard::from_bytes::<RawResponderRequestData>(&raw.data)?;
        Ok(Self {
            id: Uuid::from_slice(raw.id.as_slice())?,
            responder_id: Uuid::from_slice(raw.responder_id.as_slice())?,
            client_address: raw_data.client_address,
            method: raw_data.method,
            body: raw_data.body,
            headers: raw_data.headers,
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl<'a> TryFrom<&ResponderRequest<'a>> for RawResponderRequest {
    type Error = anyhow::Error;

    fn try_from(item: &ResponderRequest) -> Result<Self, Self::Error> {
        let raw_data = RawResponderRequestData {
            client_address: item.client_address,
            method: item.method.clone(),
            body: item.body.clone(),
            headers: item.headers.clone(),
        };

        Ok(Self {
            id: item.id.into(),
            responder_id: item.responder_id.into(),
            data: postcard::to_stdvec(&raw_data)?,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawResponderRequest;
    use crate::utils::ResponderRequest;
    use std::{
        borrow::Cow,
        net::{IpAddr, Ipv4Addr},
    };
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
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            })?,
            RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                data: vec![0, 4, 112, 111, 115, 116, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        assert_eq!(
            RawResponderRequest::try_from(&ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                method: Cow::Owned("post".to_string()),
                headers: Some(vec![(
                    Cow::Owned("Content-Type".to_string()),
                    Cow::Owned(vec![1, 2, 3]),
                )]),
                body: Some(Cow::Owned(vec![4, 5, 6])),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            })?,
            RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                data: vec![
                    1, 0, 127, 0, 0, 1, 4, 112, 111, 115, 116, 1, 1, 12, 67, 111, 110, 116, 101,
                    110, 116, 45, 84, 121, 112, 101, 3, 1, 2, 3, 1, 3, 4, 5, 6
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_responder_request() -> anyhow::Result<()> {
        assert_eq!(
            ResponderRequest::try_from(RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                data: vec![0, 4, 112, 111, 115, 116, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: None,
                method: Cow::Owned("post".to_string()),
                headers: None,
                body: None,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        assert_eq!(
            ResponderRequest::try_from(RawResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                data: vec![
                    1, 0, 127, 0, 0, 1, 4, 112, 111, 115, 116, 1, 1, 12, 67, 111, 110, 116, 101,
                    110, 116, 45, 84, 121, 112, 101, 3, 1, 2, 3, 1, 3, 4, 5, 6
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            ResponderRequest {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                responder_id: uuid!("00000000-0000-0000-0000-000000000002"),
                client_address: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                method: Cow::Owned("post".to_string()),
                headers: Some(vec![(
                    Cow::Owned("Content-Type".to_string()),
                    Cow::Owned(vec![1, 2, 3]),
                )]),
                body: Some(Cow::Owned(vec![4, 5, 6])),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}
