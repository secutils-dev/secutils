use crate::{
    users::RawSecretsAccess,
    utils::webhooks::{
        Responder, ResponderMethod, ResponderNotificationSettings, ResponderSettings,
    },
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResponder {
    pub id: Uuid,
    pub name: String,
    pub location: String,
    pub method: Vec<u8>,
    pub enabled: bool,
    pub settings: Vec<u8>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl RawResponder {
    pub fn get_raw_method(method: ResponderMethod) -> anyhow::Result<Vec<u8>> {
        Ok(postcard::to_stdvec(&method)?)
    }
}

/// Newest on-disk format for responder settings (adds the `notifications` field).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
struct RawResponderSettings {
    requests_to_track: usize,
    status_code: u16,
    body: Option<String>,
    headers: Option<Vec<(String, String)>>,
    script: Option<String>,
    secrets: RawSecretsAccess,
    notifications: Option<RawResponderNotificationSettings>,
}

/// On-disk representation of `ResponderNotificationSettings`. Kept separate from the API type so
/// the storage format can evolve independently of the public serialization.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
struct RawResponderNotificationSettings {
    throttle_seconds: u32,
}

impl From<&ResponderNotificationSettings> for RawResponderNotificationSettings {
    fn from(settings: &ResponderNotificationSettings) -> Self {
        Self {
            throttle_seconds: settings.throttle_seconds,
        }
    }
}

impl From<RawResponderNotificationSettings> for ResponderNotificationSettings {
    fn from(raw: RawResponderNotificationSettings) -> Self {
        Self {
            throttle_seconds: raw.throttle_seconds,
        }
    }
}

/// Legacy format (V2) with the `secrets` field but without `notifications`, for backward-compatible
/// deserialization of responders persisted before notifications support was added.
#[derive(Deserialize)]
struct RawResponderSettingsLegacyV2 {
    requests_to_track: usize,
    status_code: u16,
    body: Option<String>,
    headers: Option<Vec<(String, String)>>,
    script: Option<String>,
    secrets: RawSecretsAccess,
}

/// Legacy format (V1) without the `secrets` and `notifications` fields, for backward-compatible
/// deserialization of the oldest persisted responders.
#[derive(Deserialize)]
struct RawResponderSettingsLegacyV1 {
    requests_to_track: usize,
    status_code: u16,
    body: Option<String>,
    headers: Option<Vec<(String, String)>>,
    script: Option<String>,
}

fn deserialize_settings(bytes: &[u8]) -> anyhow::Result<RawResponderSettings> {
    // Postcard is positional and not self-describing, so older (shorter) buffers fail to parse as
    // the newest struct. Decode newest-first and fall back through each prior generation.
    if let Ok(settings) = postcard::from_bytes::<RawResponderSettings>(bytes) {
        return Ok(settings);
    }

    if let Ok(legacy) = postcard::from_bytes::<RawResponderSettingsLegacyV2>(bytes) {
        return Ok(RawResponderSettings {
            requests_to_track: legacy.requests_to_track,
            status_code: legacy.status_code,
            body: legacy.body,
            headers: legacy.headers,
            script: legacy.script,
            secrets: legacy.secrets,
            notifications: None,
        });
    }

    let legacy = postcard::from_bytes::<RawResponderSettingsLegacyV1>(bytes)?;
    Ok(RawResponderSettings {
        requests_to_track: legacy.requests_to_track,
        status_code: legacy.status_code,
        body: legacy.body,
        headers: legacy.headers,
        script: legacy.script,
        secrets: RawSecretsAccess::None,
        notifications: None,
    })
}

impl TryFrom<RawResponder> for Responder {
    type Error = anyhow::Error;

    fn try_from(raw: RawResponder) -> Result<Self, Self::Error> {
        let raw_settings = deserialize_settings(&raw.settings)?;
        Ok(Responder {
            id: raw.id,
            name: raw.name,
            location: raw.location.parse()?,
            method: postcard::from_bytes::<ResponderMethod>(&raw.method)?,
            enabled: raw.enabled,
            settings: ResponderSettings {
                requests_to_track: raw_settings.requests_to_track,
                status_code: raw_settings.status_code,
                body: raw_settings.body,
                headers: raw_settings.headers,
                script: raw_settings.script,
                secrets: raw_settings.secrets.into(),
                notifications: raw_settings
                    .notifications
                    .map(ResponderNotificationSettings::from),
            },
            created_at: raw.created_at,
            tags: vec![],
            updated_at: raw.updated_at,
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
            secrets: RawSecretsAccess::from(&item.settings.secrets),
            notifications: item
                .settings
                .notifications
                .as_ref()
                .map(RawResponderNotificationSettings::from),
        };

        Ok(RawResponder {
            id: item.id,
            name: item.name.clone(),
            location: item.location.to_string(),
            method: postcard::to_stdvec(&item.method)?,
            enabled: item.enabled,
            settings: postcard::to_stdvec(&raw_settings)?,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::SecretsAccess,
        utils::webhooks::{
            Responder, ResponderLocation, ResponderMethod, ResponderNotificationSettings,
            ResponderPathType, ResponderSettings, database_ext::raw_responder::RawResponder,
        },
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_raw_responder() -> anyhow::Result<()> {
        assert_eq!(
            RawResponder::try_from(&Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                    notifications: None,
                },
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            RawResponder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                location: ":=:/".to_string(),
                method: vec![0],
                enabled: true,
                settings: vec![0, 200, 1, 0, 0, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_responder() -> anyhow::Result<()> {
        assert_eq!(
            Responder::try_from(RawResponder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                location: ":=:/".to_string(),
                method: vec![0],
                enabled: true,
                settings: vec![0, 200, 1, 0, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            Responder {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "res".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                    notifications: None,
                },
                tags: vec![],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }

    #[test]
    fn bwc_deserializes_legacy_settings_without_secrets() -> anyhow::Result<()> {
        // Old format: [0, 200, 1, 0, 0, 0] = requests_to_track=0, status_code=200,
        // body=None, headers=None, script=None (no secrets field).
        let legacy_settings_bytes = vec![0, 200, 1, 0, 0, 0];
        let responder = Responder::try_from(RawResponder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "legacy".to_string(),
            location: ":=:/".to_string(),
            method: vec![0],
            enabled: true,
            settings: legacy_settings_bytes,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        })?;

        assert_eq!(responder.settings.secrets, SecretsAccess::None);
        assert_eq!(responder.settings.status_code, 200);
        assert_eq!(responder.settings.requests_to_track, 0);
        assert!(responder.settings.body.is_none());
        assert!(responder.settings.headers.is_none());
        assert!(responder.settings.script.is_none());

        Ok(())
    }

    #[test]
    fn bwc_deserializes_legacy_settings_with_body_and_headers() -> anyhow::Result<()> {
        // Old format with body="body", headers=[("key","value")], script set.
        let legacy_settings_bytes = vec![
            3, 200, 1, 1, 4, 98, 111, 100, 121, 1, 1, 3, 107, 101, 121, 5, 118, 97, 108, 117, 101,
            1, 31, 114, 101, 116, 117, 114, 110, 32, 123, 32, 98, 111, 100, 121, 58, 32, 96, 99,
            117, 115, 116, 111, 109, 32, 98, 111, 100, 121, 96, 32, 125, 59,
        ];
        let responder = Responder::try_from(RawResponder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "legacy-full".to_string(),
            location: "sub:^:/path".to_string(),
            method: vec![7],
            enabled: false,
            settings: legacy_settings_bytes,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        })?;

        assert_eq!(responder.settings.secrets, SecretsAccess::None);
        assert_eq!(responder.settings.requests_to_track, 3);
        assert_eq!(responder.settings.status_code, 200);
        assert_eq!(responder.settings.body.as_deref(), Some("body"));
        assert_eq!(
            responder.settings.headers.as_deref(),
            Some([("key".to_string(), "value".to_string())].as_slice())
        );
        assert_eq!(
            responder.settings.script.as_deref(),
            Some("return { body: `custom body` };")
        );

        Ok(())
    }

    #[test]
    fn round_trip_with_secrets_access() -> anyhow::Result<()> {
        let original = Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "with-secrets".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/api".to_string(),
                subdomain_prefix: None,
            },
            method: ResponderMethod::Get,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 0,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::Selected {
                    secrets: vec!["API_KEY".into(), "TOKEN".into()],
                },
                notifications: None,
            },
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };

        let raw = RawResponder::try_from(&original)?;
        let restored = Responder::try_from(raw)?;
        assert_eq!(restored.settings.secrets, original.settings.secrets);

        Ok(())
    }

    #[test]
    fn round_trip_with_notifications() -> anyhow::Result<()> {
        let original = Responder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "with-notifications".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/api".to_string(),
                subdomain_prefix: None,
            },
            method: ResponderMethod::Get,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 5,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: SecretsAccess::None,
                notifications: Some(ResponderNotificationSettings {
                    throttle_seconds: 3600,
                }),
            },
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };

        let raw = RawResponder::try_from(&original)?;
        let restored = Responder::try_from(raw)?;
        assert_eq!(
            restored.settings.notifications,
            original.settings.notifications
        );

        Ok(())
    }

    #[test]
    fn bwc_deserializes_legacy_v2_settings_with_secrets_without_notifications() -> anyhow::Result<()>
    {
        // V2 format: [requests_to_track=0, status_code=200, body=None, headers=None, script=None,
        // secrets=None] — has the secrets field but no notifications field.
        let legacy_settings_bytes = vec![0, 200, 1, 0, 0, 0, 0];
        let responder = Responder::try_from(RawResponder {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "legacy-v2".to_string(),
            location: ":=:/".to_string(),
            method: vec![0],
            enabled: true,
            settings: legacy_settings_bytes,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        })?;

        assert_eq!(responder.settings.secrets, SecretsAccess::None);
        assert_eq!(responder.settings.status_code, 200);
        assert!(responder.settings.notifications.is_none());

        Ok(())
    }
}
