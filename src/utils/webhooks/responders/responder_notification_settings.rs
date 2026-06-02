use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Settings that control whether and how a user is notified when their responder is hit.
///
/// The presence of these settings on a responder means notifications are enabled; their absence
/// means notifications are disabled (the default). The `throttle_seconds` value is validated
/// against the per-subscription allow-list in
/// [`SubscriptionWebhooksConfig::notification_throttle_presets`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({ "throttleSeconds": 3600 }))]
pub struct ResponderNotificationSettings {
    /// Minimum interval, in seconds, between consecutive notification emails for the responder.
    /// Requests received within a single window are coalesced into one email.
    pub throttle_seconds: u32,
}

#[cfg(test)]
mod tests {
    use super::ResponderNotificationSettings;
    use crate::tests::schema_example;
    use insta::assert_json_snapshot;

    #[test]
    fn schema_example_is_valid() {
        let example: ResponderNotificationSettings =
            serde_json::from_value(schema_example::<ResponderNotificationSettings>()).unwrap();
        assert!(example.throttle_seconds > 0);
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderNotificationSettings { throttle_seconds: 3600 }, @r###"
        {
          "throttleSeconds": 3600
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderNotificationSettings>(r#"{ "throttleSeconds": 900 }"#)?,
            ResponderNotificationSettings {
                throttle_seconds: 900
            }
        );

        Ok(())
    }
}
