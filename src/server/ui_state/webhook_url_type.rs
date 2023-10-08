use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Describes how UI should construct webhook URLs. The server supports all types of URL simultaneously.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WebhookUrlType {
    /// The webhook URL is constructed as a sub-path of the base server API URL.
    /// Example: `https://secutils.dev/api/webhooks/{user_handle}/{webhook_path}`.
    Path,
    /// The webhook URL is constructed using dedicated user `*.webhooks` subdomains.
    /// Example: `https://{user_handle}.webhooks.secutils.dev/{webhook_path}`. In this case,
    /// Secutils.dev server should be hosted behind a reverse proxy that will route requests to
    /// `https://secutils.dev/api/webhooks` endpoint preserving original full host name in the
    /// `X-Forwarded-Host` header, and the full original path in the `X-Replaced-Path` header.
    Subdomain,
}

impl FromStr for WebhookUrlType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(serde_json::from_str(&format!(r#""{s}""#))?)
    }
}

#[cfg(test)]
mod tests {
    use crate::server::WebhookUrlType;
    use std::str::FromStr;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(serde_json::to_string(&WebhookUrlType::Path)?, r#""path""#);
        assert_eq!(
            serde_json::to_string(&WebhookUrlType::Subdomain)?,
            r#""subdomain""#
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebhookUrlType>(r#""path""#)?,
            WebhookUrlType::Path
        );
        assert_eq!(
            serde_json::from_str::<WebhookUrlType>(r#""subdomain""#)?,
            WebhookUrlType::Subdomain
        );

        Ok(())
    }

    #[test]
    fn parsing_from_string() -> anyhow::Result<()> {
        assert_eq!(WebhookUrlType::from_str("path")?, WebhookUrlType::Path);
        assert_eq!(
            WebhookUrlType::from_str("subdomain")?,
            WebhookUrlType::Subdomain
        );

        Ok(())
    }
}
