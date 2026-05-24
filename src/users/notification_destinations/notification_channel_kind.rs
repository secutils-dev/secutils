use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Channel discriminator for a [`super::UserNotificationDestination`].
///
/// Marked `#[non_exhaustive]` so future channels (Slack, PagerDuty, generic webhook) widen the
/// enum without forcing every consumer to update its `match` arms in lock-step.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NotificationChannelKind {
    /// Email channel. v1 only ships this variant.
    Email,
}

impl NotificationChannelKind {
    /// Database string representation. Mirrors the `kind` `CHECK` constraint in
    /// `migrations/20260523120000_user_notification_destinations.sql`.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            NotificationChannelKind::Email => "email",
        }
    }

    /// Inverse of [`Self::as_db_str`]. Errors on unknown values so a hand-edited row in the
    /// database (or a future migration that forgot to update this method) surfaces loudly
    /// instead of being silently dropped.
    pub fn from_db_str(value: &str) -> anyhow::Result<Self> {
        match value {
            "email" => Ok(NotificationChannelKind::Email),
            other => anyhow::bail!("Unknown notification channel kind: {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NotificationChannelKind;

    #[test]
    fn channel_kind_db_round_trip() {
        let kind = NotificationChannelKind::Email;
        assert_eq!(kind.as_db_str(), "email");
        assert_eq!(
            NotificationChannelKind::from_db_str("email").unwrap(),
            NotificationChannelKind::Email
        );
        assert!(NotificationChannelKind::from_db_str("slack").is_err());
    }
}
