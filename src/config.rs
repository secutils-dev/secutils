mod components_config;
mod database_config;
mod http_config;
mod platform_config;
mod raw_config;
mod retrack_config;
mod scheduler_jobs_config;
mod security_config;
mod smtp_catch_all_config;
mod smtp_config;
mod subscriptions_config;
mod utils_config;

use url::Url;

pub use self::{
    components_config::ComponentsConfig,
    database_config::DatabaseConfig,
    http_config::HttpConfig,
    platform_config::PlatformConfig,
    raw_config::RawConfig,
    retrack_config::RetrackConfig,
    scheduler_jobs_config::SchedulerJobsConfig,
    security_config::SecurityConfig,
    smtp_catch_all_config::SmtpCatchAllConfig,
    smtp_config::SmtpConfig,
    subscriptions_config::{
        SubscriptionCertificatesConfig, SubscriptionConfig, SubscriptionScriptsConfig,
        SubscriptionSecretsConfig, SubscriptionWebScrapingConfig, SubscriptionWebSecurityConfig,
        SubscriptionWebhooksConfig, SubscriptionsConfig,
    },
    utils_config::UtilsConfig,
};

/// Secutils.dev user agent name used for all HTTP requests.
pub static SECUTILS_USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Main server config.
#[derive(Clone, Debug)]
pub struct Config {
    /// External/public URL through which service is being accessed.
    pub public_url: Url,
    /// Database configuration.
    pub db: DatabaseConfig,
    /// Security configuration (session, built-in users etc.).
    pub security: SecurityConfig,
    /// Configuration for the utility functions.
    pub utils: UtilsConfig,
    /// Configuration for the SMTP functionality.
    pub smtp: Option<SmtpConfig>,
    /// Configuration for the HTTP functionality.
    pub http: HttpConfig,
    /// Configuration for the components that are deployed separately.
    pub components: ComponentsConfig,
    /// Configuration for the scheduler jobs.
    pub scheduler: SchedulerJobsConfig,
    /// Configuration related to the Secutils.dev subscriptions.
    pub subscriptions: SubscriptionsConfig,
    /// Configuration for the Retrack service.
    pub retrack: RetrackConfig,
    /// Platform-level configuration (limits, settings exposed to UI).
    pub platform: PlatformConfig,
}

impl AsRef<Config> for Config {
    fn as_ref(&self) -> &Config {
        self
    }
}

impl From<RawConfig> for Config {
    fn from(raw_config: RawConfig) -> Self {
        Self {
            public_url: raw_config.public_url,
            db: raw_config.db,
            security: raw_config.security,
            smtp: raw_config.smtp,
            http: raw_config.http,
            components: raw_config.components,
            subscriptions: raw_config.subscriptions,
            utils: raw_config.utils,
            scheduler: raw_config.scheduler,
            retrack: raw_config.retrack,
            platform: raw_config.platform,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, RawConfig, SmtpCatchAllConfig, SmtpConfig};
    use insta::assert_debug_snapshot;
    use regex::Regex;
    use url::Url;

    #[test]
    fn conversion_from_raw_config() {
        let mut raw_config = RawConfig::default();
        raw_config.subscriptions.feature_overview_url =
            Some(Url::parse("http://localhost:7272").unwrap());
        raw_config.smtp = Some(SmtpConfig {
            username: "test@secutils.dev".to_string(),
            password: "password".to_string(),
            address: "smtp.secutils.dev".to_string(),
            catch_all: Some(SmtpCatchAllConfig {
                recipient: "test@secutils.dev".to_string(),
                text_matcher: Regex::new(r"test").unwrap(),
            }),
        });

        assert_debug_snapshot!(Config::from(raw_config), @r###"
        Config {
            public_url: Url {
                scheme: "http",
                cannot_be_a_base: false,
                username: "",
                password: None,
                host: Some(
                    Domain(
                        "localhost",
                    ),
                ),
                port: Some(
                    7070,
                ),
                path: "/",
                query: None,
                fragment: None,
            },
            db: DatabaseConfig {
                name: "secutils",
                host: "localhost",
                port: 5432,
                username: "postgres",
                password: None,
                max_connections: 100,
                min_connections: 5,
                acquire_timeout: 10s,
                max_lifetime: 1800s,
                idle_timeout: 600s,
            },
            security: SecurityConfig {
                session_cookie_name: "id",
                jwt_secret: None,
                secrets_encryption_key: None,
                operators: None,
                preconfigured_users: None,
                max_user_api_keys: 30,
            },
            utils: UtilsConfig {
                diff_context_radius: 3,
                max_responder_body_size: 10485760,
            },
            smtp: Some(
                SmtpConfig {
                    username: "test@secutils.dev",
                    password: "password",
                    address: "smtp.secutils.dev",
                    catch_all: Some(
                        SmtpCatchAllConfig {
                            recipient: "test@secutils.dev",
                            text_matcher: Regex(
                                "test",
                            ),
                        },
                    ),
                },
            ),
            http: HttpConfig {
                client: HttpClientConfig {
                    timeout: 30s,
                    pool_idle_timeout: 5s,
                    max_retries: 3,
                    verbose: false,
                },
            },
            components: ComponentsConfig {
                kratos_url: Url {
                    scheme: "http",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "localhost",
                        ),
                    ),
                    port: Some(
                        4433,
                    ),
                    path: "/",
                    query: None,
                    fragment: None,
                },
                kratos_admin_url: Url {
                    scheme: "http",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "localhost",
                        ),
                    ),
                    port: Some(
                        4434,
                    ),
                    path: "/",
                    query: None,
                    fragment: None,
                },
                search_index_version: 4,
            },
            scheduler: SchedulerJobsConfig {
                notifications_send: "0/30 * * * * *",
                webhooks_kv_sweep: "0 */5 * * * *",
                responders_notify: "0 * * * * *",
            },
            subscriptions: SubscriptionsConfig {
                manage_url: None,
                feature_overview_url: Some(
                    Url {
                        scheme: "http",
                        cannot_be_a_base: false,
                        username: "",
                        password: None,
                        host: Some(
                            Domain(
                                "localhost",
                            ),
                        ),
                        port: Some(
                            7272,
                        ),
                        path: "/",
                        query: None,
                        fragment: None,
                    },
                ),
                basic: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 30,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                        restrict_to_public_urls: true,
                        max_proxy_response_size: 10485760,
                        max_concurrent_responder_requests: 10,
                        max_tracked_response_size: 1048576,
                        max_proxy_request_timeout: 30s,
                        responder_kv_max_key_bytes: 256,
                        responder_kv_max_value_bytes: 1048576,
                        responder_kv_max_entries: 100000,
                        responder_kv_max_total_bytes: 1073741824,
                        responder_kv_max_ttl_sec: 2592000,
                        responder_kv_max_lifespan_sec: 0,
                        responder_kv_ops_per_script: 200,
                        notification_throttle_presets: [
                            300,
                            900,
                            3600,
                            21600,
                            86400,
                        ],
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 30,
                        tracker_schedules: None,
                        min_schedule_interval: 10s,
                        max_debug_screenshots_total_size: 5242880,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 100,
                        templates: 1000,
                        private_key_algorithms: None,
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                    secrets: SubscriptionSecretsConfig {
                        max_secrets: 100,
                    },
                    scripts: SubscriptionScriptsConfig {
                        max_scripts: 100,
                    },
                },
                standard: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 30,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                        restrict_to_public_urls: true,
                        max_proxy_response_size: 10485760,
                        max_concurrent_responder_requests: 10,
                        max_tracked_response_size: 1048576,
                        max_proxy_request_timeout: 30s,
                        responder_kv_max_key_bytes: 256,
                        responder_kv_max_value_bytes: 1048576,
                        responder_kv_max_entries: 100000,
                        responder_kv_max_total_bytes: 1073741824,
                        responder_kv_max_ttl_sec: 2592000,
                        responder_kv_max_lifespan_sec: 0,
                        responder_kv_ops_per_script: 200,
                        notification_throttle_presets: [
                            300,
                            900,
                            3600,
                            21600,
                            86400,
                        ],
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 30,
                        tracker_schedules: None,
                        min_schedule_interval: 10s,
                        max_debug_screenshots_total_size: 5242880,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 100,
                        templates: 1000,
                        private_key_algorithms: None,
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                    secrets: SubscriptionSecretsConfig {
                        max_secrets: 100,
                    },
                    scripts: SubscriptionScriptsConfig {
                        max_scripts: 100,
                    },
                },
                professional: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 30,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                        restrict_to_public_urls: true,
                        max_proxy_response_size: 10485760,
                        max_concurrent_responder_requests: 10,
                        max_tracked_response_size: 1048576,
                        max_proxy_request_timeout: 30s,
                        responder_kv_max_key_bytes: 256,
                        responder_kv_max_value_bytes: 1048576,
                        responder_kv_max_entries: 100000,
                        responder_kv_max_total_bytes: 1073741824,
                        responder_kv_max_ttl_sec: 2592000,
                        responder_kv_max_lifespan_sec: 0,
                        responder_kv_ops_per_script: 200,
                        notification_throttle_presets: [
                            300,
                            900,
                            3600,
                            21600,
                            86400,
                        ],
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 30,
                        tracker_schedules: None,
                        min_schedule_interval: 10s,
                        max_debug_screenshots_total_size: 5242880,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 100,
                        templates: 1000,
                        private_key_algorithms: None,
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                    secrets: SubscriptionSecretsConfig {
                        max_secrets: 100,
                    },
                    scripts: SubscriptionScriptsConfig {
                        max_scripts: 100,
                    },
                },
                ultimate: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 30,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                        restrict_to_public_urls: true,
                        max_proxy_response_size: 10485760,
                        max_concurrent_responder_requests: 10,
                        max_tracked_response_size: 1048576,
                        max_proxy_request_timeout: 30s,
                        responder_kv_max_key_bytes: 256,
                        responder_kv_max_value_bytes: 1048576,
                        responder_kv_max_entries: 100000,
                        responder_kv_max_total_bytes: 1073741824,
                        responder_kv_max_ttl_sec: 2592000,
                        responder_kv_max_lifespan_sec: 0,
                        responder_kv_ops_per_script: 200,
                        notification_throttle_presets: [
                            300,
                            900,
                            3600,
                            21600,
                            86400,
                        ],
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 30,
                        tracker_schedules: None,
                        min_schedule_interval: 10s,
                        max_debug_screenshots_total_size: 5242880,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 100,
                        templates: 1000,
                        private_key_algorithms: None,
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                    secrets: SubscriptionSecretsConfig {
                        max_secrets: 100,
                    },
                    scripts: SubscriptionScriptsConfig {
                        max_scripts: 100,
                    },
                },
            },
            retrack: RetrackConfig {
                host: Url {
                    scheme: "http",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "localhost",
                        ),
                    ),
                    port: Some(
                        7676,
                    ),
                    path: "/",
                    query: None,
                    fragment: None,
                },
                max_webhook_body_size: 10485760,
            },
            platform: PlatformConfig {
                max_import_file_size: 10485760,
            },
        }
        "###);
    }
}
