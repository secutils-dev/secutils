use crate::utils::{Responder, ResponderMethod, ResponderSettings};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResponder {
    pub id: Vec<u8>,
    pub name: String,
    pub path: String,
    pub method: Vec<u8>,
    pub settings: Vec<u8>,
    pub created_at: i64,
}

impl RawResponder {
    pub fn get_raw_method(method: ResponderMethod) -> anyhow::Result<Vec<u8>> {
        Ok(postcard::to_stdvec(&method)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
struct RawResponderSettings {
    requests_to_track: usize,
    status_code: u16,
    body: Option<String>,
    headers: Option<Vec<(String, String)>>,
    script: Option<String>,
}

impl TryFrom<RawResponder> for Responder {
    type Error = anyhow::Error;

    fn try_from(raw: RawResponder) -> Result<Self, Self::Error> {
        let raw_settings = postcard::from_bytes::<RawResponderSettings>(&raw.settings)?;
        Ok(Responder {
            id: Uuid::from_slice(raw.id.as_slice())?,
            name: raw.name,
            path: raw.path,
            method: postcard::from_bytes::<ResponderMethod>(&raw.method)?,
            settings: ResponderSettings {
                requests_to_track: raw_settings.requests_to_track,
                status_code: raw_settings.status_code,
                body: raw_settings.body,
                headers: raw_settings.headers,
                script: raw_settings.script,
            },
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl TryFrom<&Responder> for RawResponder {
    type Error = anyhow::Error;

    fn try_from(item: &Responder) -> Result<Self, Self::Error> {
        let raw_settings = RawResponderSettings {
            requests_to_track: item.settings.requests_to_track,
            status_code: item.settings.status_code,
            body: item.settings.body.clone(),
            headers: item.settings.headers.clone(),
            script: item.settings.script.clone(),
        };

        Ok(RawResponder {
            id: item.id.into(),
            name: item.name.clone(),
            path: item.path.to_string(),
            method: postcard::to_stdvec(&item.method)?,
            settings: postcard::to_stdvec(&raw_settings)?,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        webhooks::database_ext::raw_responder::RawResponder, Responder, ResponderMethod,
        ResponderSettings,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_raw_responder() -> anyhow::Result<()> {
        assert_eq!(
            RawResponder::try_from(&Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                path: "/".to_string(),
                method: ResponderMethod::Any,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            })?,
            RawResponder {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "res".to_string(),
                path: "/".to_string(),
                method: vec![0],
                settings: vec![0, 200, 1, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        assert_eq!(
            RawResponder::try_from(&Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                path: "/path".to_string(),
                method: ResponderMethod::Connect,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: Some("return { body: `custom body` };".to_string()),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            })?,
            RawResponder {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "res".to_string(),
                path: "/path".to_string(),
                method: vec![7],
                settings: vec![
                    3, 200, 1, 1, 4, 98, 111, 100, 121, 1, 1, 3, 107, 101, 121, 5, 118, 97, 108,
                    117, 101, 1, 31, 114, 101, 116, 117, 114, 110, 32, 123, 32, 98, 111, 100, 121,
                    58, 32, 96, 99, 117, 115, 116, 111, 109, 32, 98, 111, 100, 121, 96, 32, 125,
                    59
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_responder() -> anyhow::Result<()> {
        assert_eq!(
            Responder::try_from(RawResponder {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "res".to_string(),
                path: "/".to_string(),
                method: vec![0],
                settings: vec![0, 200, 1, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                path: "/".to_string(),
                method: ResponderMethod::Any,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        assert_eq!(
            Responder::try_from(RawResponder {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "res".to_string(),
                path: "/path".to_string(),
                method: vec![7],
                settings: vec![
                    3, 200, 1, 1, 4, 98, 111, 100, 121, 1, 1, 3, 107, 101, 121, 5, 118, 97, 108,
                    117, 101, 1, 31, 114, 101, 116, 117, 114, 110, 32, 123, 32, 98, 111, 100, 121,
                    58, 32, 96, 99, 117, 115, 116, 111, 109, 32, 98, 111, 100, 121, 96, 32, 125,
                    59
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                path: "/path".to_string(),
                method: ResponderMethod::Connect,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: Some("return { body: `custom body` };".to_string()),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}
