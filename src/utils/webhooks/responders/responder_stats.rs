use serde::Serialize;
use serde_with::{TimestampSeconds, serde_as};
use time::OffsetDateTime;
use uuid::Uuid;

/// Represents a responder stat.
#[serde_as]
#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderStats {
    /// Unique responder ID that the stat belongs to (UUIDv7).
    pub responder_id: Uuid,
    /// Number of responder requests that are currently stored.
    pub request_count: usize,
    /// The timestamp when the responder was last requested.
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_requested_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::ResponderStats;
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderStats {
            responder_id: uuid!("00000000-0000-0000-0000-000000000001"),
            request_count: 10,
            last_requested_at: Some(OffsetDateTime::from_unix_timestamp(946720800)?)
        }, @r###"
        {
          "responderId": "00000000-0000-0000-0000-000000000001",
          "requestCount": 10,
          "lastRequestedAt": 946720800
        }
        "###);

        assert_json_snapshot!(ResponderStats {
            responder_id: uuid!("00000000-0000-0000-0000-000000000001"),
            request_count: 10,
            last_requested_at: None
        }, @r###"
        {
          "responderId": "00000000-0000-0000-0000-000000000001",
          "requestCount": 10
        }
        "###);

        Ok(())
    }
}
