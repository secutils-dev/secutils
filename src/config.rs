mod smtp_config;

use url::Url;

pub use self::smtp_config::SmtpConfig;

/// Main server config.
#[derive(Clone, Debug)]
pub struct Config {
    /// Version of the Secutils binary.
    pub version: String,
    /// HTTP port to bind API server to.
    pub http_port: u16,
    /// External/public URL through which service is being accessed.
    pub public_url: Url,
    /// Configuration for the SMTP functionality.
    pub smtp: Option<SmtpConfig>,
}

impl AsRef<Config> for Config {
    fn as_ref(&self) -> &Config {
        self
    }
}
