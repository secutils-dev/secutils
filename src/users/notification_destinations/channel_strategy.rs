use crate::users::NotificationChannelKind;
use anyhow::bail;

/// Maximum length of a notification email handle. RFC 5321 caps the full address at 254 octets;
/// we round down a touch to leave headroom for headers built around the address.
const MAX_EMAIL_LENGTH: usize = 254;

/// Per-channel "claim then prove control" behaviour.
///
/// Today only [`EmailChannelStrategy`] is wired up. When a second channel (Slack via OAuth,
/// PagerDuty service key handshake, signed webhook, ...) lands, implement this trait for it and
/// extend [`channel_strategy`] to dispatch on [`NotificationChannelKind`].
pub trait ChannelStrategy {
    /// Validates that `address` is a syntactically acceptable handle for this channel before any
    /// verification email/code is issued. Returns the canonical form to be persisted.
    fn canonicalize(&self, address: &str) -> anyhow::Result<String>;

    /// Renders a short user-visible label, e.g. masked email `f***@example.com`. Used in
    /// `tracing::info!` lines for support traceability without leaking the full address.
    fn mask(&self, address: &str) -> String;
}

pub struct EmailChannelStrategy;
impl ChannelStrategy for EmailChannelStrategy {
    fn canonicalize(&self, address: &str) -> anyhow::Result<String> {
        let trimmed = address.trim();
        if trimmed.is_empty() {
            bail!("Notification email cannot be empty.");
        }

        if trimmed.len() > MAX_EMAIL_LENGTH {
            bail!("Notification email is too long.");
        }

        // Minimum viable shape check: exactly one `@`, non-empty local-part, dotted host.
        // Stricter validation (RFC 5321) is enforced when lettre actually parses the address,
        // but we want to reject obvious garbage before scheduling a verification email.
        let (local, host) = trimmed.split_once('@').ok_or_else(|| {
            anyhow::anyhow!("Notification email must contain exactly one '@' separator.")
        })?;

        if local.is_empty() {
            bail!("Notification email local-part cannot be empty.");
        }

        if host.contains('@') {
            bail!("Notification email must contain exactly one '@' separator.");
        }

        if !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
            bail!("Notification email host must contain a dot and not start or end with one.");
        }

        Ok(trimmed.to_lowercase())
    }

    fn mask(&self, address: &str) -> String {
        match address.split_once('@') {
            Some((local, host)) if !local.is_empty() => {
                let first = local.chars().next().unwrap_or('*');
                format!("{first}***@{host}")
            }
            _ => "***".to_string(),
        }
    }
}

/// Returns the strategy for the given channel kind. v1 only ships email, but the dispatcher is
/// already shaped so future kinds plug in without touching call sites.
pub fn channel_strategy(kind: NotificationChannelKind) -> &'static dyn ChannelStrategy {
    match kind {
        NotificationChannelKind::Email => &EmailChannelStrategy,
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelStrategy, EmailChannelStrategy};

    #[test]
    fn canonicalizes_valid_email() {
        let strategy = EmailChannelStrategy;
        assert_eq!(
            strategy.canonicalize("Alerts@Example.COM").unwrap(),
            "alerts@example.com"
        );
        assert_eq!(
            strategy
                .canonicalize("  alerts+tag@sub.example.com  ")
                .unwrap(),
            "alerts+tag@sub.example.com"
        );
    }

    #[test]
    fn rejects_invalid_email_shapes() {
        let strategy = EmailChannelStrategy;
        for bad in [
            "",
            "   ",
            "no-at-sign",
            "@no-local.example.com",
            "double@@example.com",
            "no-dot@host",
            "trailing-dot@host.",
            "leading-dot@.host",
        ] {
            assert!(
                strategy.canonicalize(bad).is_err(),
                "expected `{bad}` to be rejected"
            );
        }
    }

    #[test]
    fn rejects_email_above_max_length() {
        let strategy = EmailChannelStrategy;
        let address = format!("{}@example.com", "a".repeat(260));
        assert!(strategy.canonicalize(&address).is_err());
    }

    #[test]
    fn masks_email_for_logging() {
        let strategy = EmailChannelStrategy;
        assert_eq!(strategy.mask("alerts@example.com"), "a***@example.com");
        assert_eq!(strategy.mask("not-an-email"), "***");
        assert_eq!(strategy.mask("@example.com"), "***");
    }
}
