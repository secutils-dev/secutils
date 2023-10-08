mod components_config;
mod scheduler_jobs_config;
mod smtp_catch_all_config;
mod smtp_config;

use crate::server::WebhookUrlType;
use url::Url;

pub use self::{
    components_config::ComponentsConfig, scheduler_jobs_config::SchedulerJobsConfig,
    smtp_catch_all_config::SmtpCatchAllConfig, smtp_config::SmtpConfig,
};

/// Main server config.
#[derive(Clone, Debug)]
pub struct Config {
    /// Version of the Secutils binary.
    pub version: String,
    /// HTTP port to bind API server to.
    pub http_port: u16,
    /// External/public URL through which service is being accessed.
    pub public_url: Url,
    /// Describes the preferred way to construct webhook URLs.
    pub webhook_url_type: WebhookUrlType,
    /// Configuration for the SMTP functionality.
    pub smtp: Option<SmtpConfig>,
    /// Configuration for the components that are deployed separately.
    pub components: ComponentsConfig,
    /// Configuration for the scheduler jobs.
    pub jobs: SchedulerJobsConfig,
}

impl AsRef<Config> for Config {
    fn as_ref(&self) -> &Config {
        self
    }
}
