use crate::config::SmtpCatchAllConfig;

/// Configuration for the SMTP functionality.
#[derive(Clone, Debug)]
pub struct SmtpConfig {
    /// Username to use to authenticate to the SMTP server.
    pub username: String,
    /// Password to use to authenticate to the SMTP server.
    pub password: String,
    /// Address of the SMTP server.
    pub address: String,
    /// Optional configuration for catch-all email recipient (used for troubleshooting only).
    pub catch_all: Option<SmtpCatchAllConfig>,
}
