use crate::users::UserId;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

/// Represents a user secret (key-value pair stored encrypted at rest).
/// The value is never returned to clients after creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSecret {
    /// Unique identifier for the secret.
    #[serde(skip)]
    pub id: Uuid,
    /// The user who owns this secret.
    #[serde(skip)]
    pub user_id: UserId,
    /// The secret name (used to reference it in scripts and templates).
    pub name: String,
    /// The encrypted value, populated only for internal use (never serialized).
    #[serde(skip)]
    pub encrypted_value: Option<Vec<u8>>,
    /// When the secret was first created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    /// When the secret value was last updated.
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}
