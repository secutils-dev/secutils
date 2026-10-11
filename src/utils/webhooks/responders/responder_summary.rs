use crate::{
    users::EntityTag,
    utils::webhooks::{ResponderLocation, ResponderMethod, ResponderStats},
};
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Lightweight responder representation returned by list endpoints. It omits the potentially
/// large body, headers, and script; use the responder detail endpoint to get the full responder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponderSummary {
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
    /// Subset of the responder settings needed to render responder lists.
    pub settings: ResponderSummarySettings,
    /// Size of the serialized (uncompressed) responder settings in bytes, including body,
    /// headers, script, and other settings. This isn't the HTTP response size or the physical
    /// storage size.
    pub settings_size: u64,
    /// Stats computed from the currently retained request history.
    pub stats: ResponderStats,
    /// Tags assigned to this responder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<EntityTag>,
    /// Date and time when the responder was created.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub created_at: OffsetDateTime,
    /// Date and time when the responder was last updated.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub updated_at: OffsetDateTime,
}

/// Subset of the responder settings included in [`ResponderSummary`].
#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponderSummarySettings {
    /// Number of requests to track.
    pub requests_to_track: usize,
    /// Indicates whether the responder has a script.
    pub has_script: bool,
    /// Indicates whether notifications are enabled for the responder.
    pub has_notifications: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::{
        ResponderLocation, ResponderMethod, ResponderPathType, ResponderStats, ResponderSummary,
        ResponderSummarySettings,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderSummary {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "some-name".to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/path".to_string(),
                subdomain_prefix: None
            },
            method: ResponderMethod::Post,
            enabled: true,
            settings: ResponderSummarySettings {
                requests_to_track: 10,
                has_script: true,
                has_notifications: false,
            },
            settings_size: 1024,
            stats: ResponderStats {
                request_count: 3,
                last_requested_at: Some(OffsetDateTime::from_unix_timestamp(946720820)?),
            },
            tags: vec![],
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
            "hasScript": true,
            "hasNotifications": false
          },
          "settingsSize": 1024,
          "stats": {
            "requestCount": 3,
            "lastRequestedAt": 946720820
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "###);

        Ok(())
    }
}
