use serde::Serialize;
use serde_with::{TimestampSeconds, serde_as};
use time::OffsetDateTime;
use utoipa::ToSchema;

/// Usage of a responder's script key-value storage (`secutils.kv.*`), counting only live
/// (non-expired) entries, together with the quotas that apply to it.
#[serde_as]
#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponderStorageStats {
    /// Number of live entries.
    pub entries: u64,
    /// Total size of live entry values in bytes.
    pub total_bytes: u64,
    /// Creation time of the oldest live entry, if any.
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<i64>)]
    pub oldest_created_at: Option<OffsetDateTime>,
    /// The earliest expiry among live entries, if any entry expires.
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<i64>)]
    pub next_expires_at: Option<OffsetDateTime>,
    /// Maximum number of live entries allowed for the responder.
    pub max_entries: u64,
    /// Maximum total size of live entry values in bytes allowed for the responder.
    pub max_total_bytes: u64,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::ResponderStorageStats;
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderStorageStats {
            entries: 3,
            total_bytes: 1024,
            oldest_created_at: Some(OffsetDateTime::from_unix_timestamp(946720800)?),
            next_expires_at: Some(OffsetDateTime::from_unix_timestamp(946720810)?),
            max_entries: 100,
            max_total_bytes: 4096,
        }, @r###"
        {
          "entries": 3,
          "totalBytes": 1024,
          "oldestCreatedAt": 946720800,
          "nextExpiresAt": 946720810,
          "maxEntries": 100,
          "maxTotalBytes": 4096
        }
        "###);

        assert_json_snapshot!(ResponderStorageStats {
            entries: 0,
            total_bytes: 0,
            oldest_created_at: None,
            next_expires_at: None,
            max_entries: 100,
            max_total_bytes: 4096,
        }, @r###"
        {
          "entries": 0,
          "totalBytes": 0,
          "maxEntries": 100,
          "maxTotalBytes": 4096
        }
        "###);

        Ok(())
    }
}
