use crate::users::{EntityTag, UserId};
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a user secret (key-value pair stored encrypted at rest).
/// The value is never returned to clients after creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSecret {
    /// Unique identifier for the secret.
    pub id: Uuid,
    /// The user who owns this secret.
    #[serde(skip)]
    pub user_id: UserId,
    /// The secret name (used to reference it in scripts and templates).
    pub name: String,
    /// The encrypted value, populated only for internal use (never serialized).
    #[serde(skip)]
    pub encrypted_value: Option<Vec<u8>>,
    /// Tags assigned to this secret.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<EntityTag>,
    /// When the secret was first created.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub created_at: OffsetDateTime,
    /// When the secret value was last updated.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub updated_at: OffsetDateTime,
}
