mod components_config;
mod raw_config;
mod scheduler_jobs_config;
mod security_config;
mod smtp_catch_all_config;
mod smtp_config;
mod subscriptions_config;
mod utils_config;

use url::Url;

pub use self::{
    components_config::ComponentsConfig,
    raw_config::RawConfig,
    scheduler_jobs_config::SchedulerJobsConfig,
    security_config::{BuiltinUserConfig, SecurityConfig, SESSION_KEY_LENGTH_BYTES},
    smtp_catch_all_config::SmtpCatchAllConfig,
    smtp_config::SmtpConfig,
    subscriptions_config::{
        SubscriptionCertificatesConfig, SubscriptionConfig, SubscriptionWebScrapingConfig,
        SubscriptionWebSecurityConfig, SubscriptionWebhooksConfig, SubscriptionsConfig,
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
    /// Configuration for the utility functions.
    pub utils: UtilsConfig,
    /// Configuration for the SMTP functionality.
    pub smtp: Option<SmtpConfig>,
    /// Configuration for the components that are deployed separately.
    pub components: ComponentsConfig,
    /// Configuration for the scheduler jobs.
    pub scheduler: SchedulerJobsConfig,
    /// Configuration related to the Secutils.dev subscriptions.
    pub subscriptions: SubscriptionsConfig,
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
            smtp: raw_config.smtp,
            components: raw_config.components,
            subscriptions: raw_config.subscriptions,
            utils: raw_config.utils,
            scheduler: raw_config.scheduler,
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
            utils: UtilsConfig {
                webhook_url_type: Subdomain,
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
            components: ComponentsConfig {
                web_scraper_url: Url {
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
                search_index_version: 3,
            },
            scheduler: SchedulerJobsConfig {
                web_page_trackers_schedule: Schedule {
                    source: "0 * * * * * *",
                    fields: ScheduleFields {
                        years: Years {
                            ordinals: None,
                        },
                        days_of_week: DaysOfWeek {
                            ordinals: None,
                        },
                        months: Months {
                            ordinals: None,
                        },
                        days_of_month: DaysOfMonth {
                            ordinals: None,
                        },
                        hours: Hours {
                            ordinals: None,
                        },
                        minutes: Minutes {
                            ordinals: None,
                        },
                        seconds: Seconds {
                            ordinals: Some(
                                {
                                    0,
                                },
                            ),
                        },
                    },
                },
                web_page_trackers_fetch: Schedule {
                    source: "0 * * * * * *",
                    fields: ScheduleFields {
                        years: Years {
                            ordinals: None,
                        },
                        days_of_week: DaysOfWeek {
                            ordinals: None,
                        },
                        months: Months {
                            ordinals: None,
                        },
                        days_of_month: DaysOfMonth {
                            ordinals: None,
                        },
                        hours: Hours {
                            ordinals: None,
                        },
                        minutes: Minutes {
                            ordinals: None,
                        },
                        seconds: Seconds {
                            ordinals: Some(
                                {
                                    0,
                                },
                            ),
                        },
                    },
                },
                notifications_send: Schedule {
                    source: "0/30 * * * * * *",
                    fields: ScheduleFields {
                        years: Years {
                            ordinals: None,
                        },
                        days_of_week: DaysOfWeek {
                            ordinals: None,
                        },
                        months: Months {
                            ordinals: None,
                        },
                        days_of_month: DaysOfMonth {
                            ordinals: None,
                        },
                        hours: Hours {
                            ordinals: None,
                        },
                        minutes: Minutes {
                            ordinals: None,
                        },
                        seconds: Seconds {
                            ordinals: Some(
                                {
                                    0,
                                    30,
                                },
                            ),
                        },
                    },
                },
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
                        responder_requests: 100,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 100,
                        tracker_schedules: None,
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
                },
                standard: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 100,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 100,
                        tracker_schedules: None,
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
                },
                professional: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 100,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 100,
                        tracker_schedules: None,
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
                },
                ultimate: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 100,
                        responder_requests: 100,
                        js_runtime_heap_size: 10485760,
                        js_runtime_script_execution_time: 30s,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 100,
                        tracker_revisions: 100,
                        tracker_schedules: None,
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
                },
            },
        }
        "###);
    }
}
