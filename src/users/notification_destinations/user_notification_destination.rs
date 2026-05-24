use crate::users::{UserId, notification_destinations::NotificationChannelKind};
use serde::Serialize;
use serde_with::{TimestampSeconds, serde_as};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Domain representation of a row in `user_notification_destinations`. Mirrors every column
/// on the table; secret fields are kept off the wire via `#[serde(skip)]`. Computed flags
/// (`is_verified`, `is_unsubscribed`, `verification_pending`) are exposed as inherent methods
/// rather than stored fields so consumers always see the right answer for the current `now`.
#[serde_as]
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserNotificationDestination {
    pub id: Uuid,
    #[serde(skip)]
    pub user_id: UserId,
    pub kind: NotificationChannelKind,
    /// The channel-native handle. For email destinations this is the lowercased address.
    pub address: String,
    /// Channel-specific configuration. Currently always `{}`; reserved for future channels
    /// that need additional parameters (e.g. Slack workspace + channel ID).
    #[serde(skip)]
    #[allow(dead_code)]
    pub config: serde_json::Value,
    /// Argon2 PHC hash of the active verification code, if any.
    #[serde(skip)]
    pub verification_code_hash: Option<String>,
    /// Number of failed code-entry attempts against the active code.
    #[serde(skip)]
    pub verification_attempts: i32,
    /// Opaque token that grants one-click unsubscribe via `/api/notifications/unsubscribe`.
    /// Treated as bearer-equivalent for the destination it identifies.
    #[serde(skip)]
    pub unsubscribe_token: String,

    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<OffsetDateTime>,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_expires_at: Option<OffsetDateTime>,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_sent_at: Option<OffsetDateTime>,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsubscribed_at: Option<OffsetDateTime>,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub created_at: OffsetDateTime,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub updated_at: OffsetDateTime,
}

impl UserNotificationDestination {
    /// True once the recipient has proven control of the destination.
    pub fn is_verified(&self) -> bool {
        self.verified_at.is_some()
    }

    /// True after the recipient has used the one-click unsubscribe link.
    pub fn is_unsubscribed(&self) -> bool {
        self.unsubscribed_at.is_some()
    }

    /// True while a verification code is outstanding (issued, not yet entered, not expired).
    pub fn has_pending_verification(&self, now: OffsetDateTime) -> bool {
        match (
            self.verification_code_hash.as_ref(),
            self.verification_expires_at,
        ) {
            (Some(_), Some(expires_at)) => expires_at > now,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;
    use uuid::uuid;

    fn fixture() -> UserNotificationDestination {
        UserNotificationDestination {
            id: uuid!("00000000-0000-0000-0000-000000000010"),
            user_id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com".to_string(),
            config: serde_json::json!({}),
            verification_code_hash: Some("$argon2id$v=19$m=...$...".to_string()),
            verification_attempts: 2,
            unsubscribe_token: "tok-secret".to_string(),
            verified_at: Some(OffsetDateTime::from_unix_timestamp(1700000100).unwrap()),
            verification_expires_at: Some(OffsetDateTime::from_unix_timestamp(1700001000).unwrap()),
            verification_sent_at: Some(OffsetDateTime::from_unix_timestamp(1700000000).unwrap()),
            unsubscribed_at: None,
            created_at: OffsetDateTime::from_unix_timestamp(1700000000).unwrap(),
            updated_at: OffsetDateTime::from_unix_timestamp(1700000100).unwrap(),
        }
    }

    #[test]
    fn serializes_camel_case_and_skips_secrets() {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(fixture(), @r###"
            {
              "id": "00000000-0000-0000-0000-000000000010",
              "kind": "email",
              "address": "alerts@example.com",
              "verifiedAt": 1700000100,
              "verificationExpiresAt": 1700001000,
              "verificationSentAt": 1700000000,
              "createdAt": 1700000000,
              "updatedAt": 1700000100
            }
            "###);
        });
    }

    #[test]
    fn computed_flags() {
        let mut dest = fixture();
        assert!(dest.is_verified());
        assert!(!dest.is_unsubscribed());
        assert!(
            dest.has_pending_verification(OffsetDateTime::from_unix_timestamp(1700000500).unwrap())
        );
        assert!(
            !dest
                .has_pending_verification(OffsetDateTime::from_unix_timestamp(1700001001).unwrap())
        );

        dest.verified_at = None;
        assert!(!dest.is_verified());
        dest.unsubscribed_at = Some(OffsetDateTime::from_unix_timestamp(1700001000).unwrap());
        assert!(dest.is_unsubscribed());

        dest.verification_code_hash = None;
        assert!(
            !dest
                .has_pending_verification(OffsetDateTime::from_unix_timestamp(1700000500).unwrap())
        );
    }
}
