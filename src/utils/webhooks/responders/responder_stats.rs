use serde::Serialize;
use serde_with::{TimestampSeconds, serde_as};
use time::OffsetDateTime;
use utoipa::ToSchema;

/// Responder request stats, computed from the currently retained request history (not lifetime
/// totals): clearing the history resets them.
#[serde_as]
#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponderStats {
    /// Number of responder requests that are currently stored.
    pub request_count: usize,
    /// The timestamp of the most recent stored request, if any.
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<i64>)]
    pub last_requested_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::ResponderStats;
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderStats {
            request_count: 10,
            last_requested_at: Some(OffsetDateTime::from_unix_timestamp(946720800)?)
        }, @r###"
        {
          "requestCount": 10,
          "lastRequestedAt": 946720800
        }
        "###);

        assert_json_snapshot!(ResponderStats {
            request_count: 0,
            last_requested_at: None
        }, @r###"
        {
          "requestCount": 0
        }
        "###);

        Ok(())
    }
}
