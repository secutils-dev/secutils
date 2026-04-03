use crate::users::UserId;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a user API key for programmatic access to the Secutils API.
/// The plaintext token is only returned once at creation or regeneration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserApiKey {
    /// Unique identifier for the API key.
    pub id: Uuid,
    /// The user who owns this API key.
    #[serde(skip)]
    pub user_id: UserId,
    /// A user-assigned label for this key.
    pub name: String,
    /// SHA-256 hash of the plaintext token (never serialized).
    #[serde(skip)]
    pub token_hash: Vec<u8>,
    /// When the API key was created.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub created_at: OffsetDateTime,
    /// When the API key metadata was last updated.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub updated_at: OffsetDateTime,
    /// When the API key expires, or `None` if it never expires.
    #[serde(with = "time::serde::timestamp::option")]
    #[schema(value_type = Option<i64>)]
    pub expires_at: Option<OffsetDateTime>,
    /// When the API key was last used for authentication.
    #[serde(with = "time::serde::timestamp::option")]
    #[schema(value_type = Option<i64>)]
    pub last_used_at: Option<OffsetDateTime>,
}

/// Response returned when creating or regenerating an API key. Includes the
/// plaintext token which is shown exactly once.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCreateResponse {
    #[serde(flatten)]
    pub api_key: UserApiKey,
    /// The plaintext API key token. Store it securely - it cannot be retrieved again.
    pub token: String,
}
