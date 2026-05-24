mod api_ext;
mod channel_strategy;
mod database_ext;
mod notification_channel_kind;
mod user_notification_destination;

pub use self::{
    api_ext::{
        NotificationEmailSetParams, NotificationEmailVerifyParams, ResolvedRecipient,
        resolve_recipient_for_user_id, unsubscribe_url,
    },
    notification_channel_kind::NotificationChannelKind,
    user_notification_destination::UserNotificationDestination,
};

/// Test-only re-exports of internal helpers needed by cross-module tests (e.g. notifications
/// integration tests that need to seed a verified destination row directly through the DB).
/// Keeping these behind `#[cfg(test)]` avoids leaking the raw upsert/expiry surface into
/// production code.
#[cfg(test)]
pub mod tests {
    pub(crate) use super::database_ext::{PendingDestinationUpsert, verification_expiry};
}
