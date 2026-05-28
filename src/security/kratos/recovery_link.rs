use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

/// Kratos admin-issued recovery link returned by `POST /admin/recovery/code` (or the legacy
/// `POST /admin/recovery/link`).
///
/// The operator hands this URL to a user (or, in the clone case, opens it themselves) to set a
/// password and log in. When Kratos is configured with `recovery.use = "code"` (the modern
/// default), the URL points at the recovery flow page and `recovery_code` carries the one-time code
/// the user has to enter there. With `use = "link"`, the URL is self-contained and `recovery_code` is absent.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecoveryLink {
    /// Single-use Kratos URL that starts the recovery flow.
    pub recovery_link: String,
    /// One-time code the user enters at `recovery_link` to complete recovery. Only present
    /// when Kratos uses the `code` recovery strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_code: Option<String>,
    /// When the link/code expires (Kratos returns RFC3339).
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String)]
    pub expires_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::security::kratos::RecoveryLink;
    use time::OffsetDateTime;

    #[test]
    fn deserialization_link_strategy() -> anyhow::Result<()> {
        let link: RecoveryLink = serde_json::from_str(
            r#"
{
    "recovery_link": "http://127.0.0.1:7171/self-service/recovery?flow=abc&token=xyz",
    "expires_at": "2030-01-01T10:00:00Z"
}
            "#,
        )?;
        assert_eq!(
            link.recovery_link,
            "http://127.0.0.1:7171/self-service/recovery?flow=abc&token=xyz"
        );
        assert!(link.recovery_code.is_none());
        assert_eq!(
            link.expires_at,
            OffsetDateTime::from_unix_timestamp(1893492000)?
        );
        Ok(())
    }

    #[test]
    fn deserialization_code_strategy() -> anyhow::Result<()> {
        let link: RecoveryLink = serde_json::from_str(
            r#"
{
    "recovery_link": "http://127.0.0.1:7171/self-service/recovery?flow=abc",
    "recovery_code": "123456",
    "expires_at": "2030-01-01T10:00:00Z"
}
            "#,
        )?;
        assert_eq!(link.recovery_code.as_deref(), Some("123456"));
        Ok(())
    }
}
