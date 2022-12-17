mod smtp_config;

pub use self::smtp_config::SmtpConfig;

/// Main server config.
#[derive(Clone, Debug)]
pub struct Config {
    /// HTTP port to bind API server to.
    pub http_port: u16,
    /// Configuration for the SMTP functionality.
    pub smtp: Option<SmtpConfig>,
}

impl AsRef<Config> for Config {
    fn as_ref(&self) -> &Config {
        self
    }
}
