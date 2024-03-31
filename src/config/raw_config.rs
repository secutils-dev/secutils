use crate::config::{
    database_config::DatabaseConfig, utils_config::UtilsConfig, ComponentsConfig,
    SchedulerJobsConfig, SecurityConfig, SmtpConfig, SubscriptionsConfig,
};
use figment::{providers, providers::Format, value, Figment, Metadata, Profile, Provider};
use serde_derive::{Deserialize, Serialize};
use url::Url;

/// Raw configuration structure that is used to read the configuration from the file.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct RawConfig {
    /// Defines a TCP port to listen on.
    pub port: u16,
    /// External/public URL through which service is being accessed.
    pub public_url: Url,
    /// Database configuration.
    pub db: DatabaseConfig,
    /// Security configuration (session, built-in users etc.).
    pub security: SecurityConfig,
    /// Configuration for the components that are deployed separately.
    pub components: ComponentsConfig,
    /// Configuration for the scheduler jobs.
    pub scheduler: SchedulerJobsConfig,
    /// Configuration related to the Secutils.dev subscriptions.
    pub subscriptions: SubscriptionsConfig,
    /// Configuration for the utilities.
    pub utils: UtilsConfig,
    /// Configuration for the SMTP functionality.
    pub smtp: Option<SmtpConfig>,
}

impl RawConfig {
    /// Reads the configuration from the file (TOML) and merges it with the default values.
    pub fn read_from_file(path: &str) -> anyhow::Result<Self> {
        Ok(Figment::from(RawConfig::default())
            .merge(providers::Toml::file(path))
            .extract()?)
    }
}

impl Default for RawConfig {
    fn default() -> Self {
        let port = 7070;
        Self {
            port,
            db: DatabaseConfig::default(),
            public_url: Url::parse(&format!("http://localhost:{port}"))
                .expect("Cannot parse public URL parameter."),
            security: SecurityConfig::default(),
            components: ComponentsConfig::default(),
            scheduler: SchedulerJobsConfig::default(),
            subscriptions: SubscriptionsConfig::default(),
            utils: UtilsConfig::default(),
            smtp: None,
        }
    }
}

impl Provider for RawConfig {
    fn metadata(&self) -> Metadata {
        Metadata::named("Secutils.dev main configuration")
    }

    fn data(&self) -> Result<value::Map<Profile, value::Dict>, figment::Error> {
        providers::Serialized::defaults(Self::default()).data()
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{RawConfig, SESSION_KEY_LENGTH_BYTES};
    use insta::{assert_debug_snapshot, assert_toml_snapshot};
    use url::Url;

    #[test]
    fn serialization_and_default() {
        let mut default_config = RawConfig::default();
        default_config.security.session_key = "a".repeat(SESSION_KEY_LENGTH_BYTES);
        default_config.subscriptions.feature_overview_url =
            Some(Url::parse("http://localhost:7272").unwrap());

        assert_toml_snapshot!(default_config, @r###"
        port = 7070
        public-url = 'http://localhost:7070/'

        [db]
        name = 'secutils'
        host = 'localhost'
        port = 5432
        username = 'postgres'

        [security]
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = false

        [components]
        web-scraper-url = 'http://localhost:7272/'
        search-index-version = 4

        [scheduler]
        web-page-trackers-schedule = '0 * * * * * *'
        web-page-trackers-fetch = '0 * * * * * *'
        notifications-send = '0/30 * * * * * *'

        [subscriptions]
        feature-overview-url = 'http://localhost:7272/'
        [subscriptions.basic.webhooks]
        responders = 100
        responder-requests = 30
        js-runtime-heap-size = 10485760
        js-runtime-script-execution-time = 30000

        [subscriptions.basic.web-scraping]
        trackers = 100
        tracker-revisions = 30

        [subscriptions.basic.certificates]
        private-keys = 100
        templates = 1000

        [subscriptions.basic.web-security]
        policies = 1000
        import-policy-from-url = true
        [subscriptions.standard.webhooks]
        responders = 100
        responder-requests = 30
        js-runtime-heap-size = 10485760
        js-runtime-script-execution-time = 30000

        [subscriptions.standard.web-scraping]
        trackers = 100
        tracker-revisions = 30

        [subscriptions.standard.certificates]
        private-keys = 100
        templates = 1000

        [subscriptions.standard.web-security]
        policies = 1000
        import-policy-from-url = true
        [subscriptions.professional.webhooks]
        responders = 100
        responder-requests = 30
        js-runtime-heap-size = 10485760
        js-runtime-script-execution-time = 30000

        [subscriptions.professional.web-scraping]
        trackers = 100
        tracker-revisions = 30

        [subscriptions.professional.certificates]
        private-keys = 100
        templates = 1000

        [subscriptions.professional.web-security]
        policies = 1000
        import-policy-from-url = true
        [subscriptions.ultimate.webhooks]
        responders = 100
        responder-requests = 30
        js-runtime-heap-size = 10485760
        js-runtime-script-execution-time = 30000

        [subscriptions.ultimate.web-scraping]
        trackers = 100
        tracker-revisions = 30

        [subscriptions.ultimate.certificates]
        private-keys = 100
        templates = 1000

        [subscriptions.ultimate.web-security]
        policies = 1000
        import-policy-from-url = true

        [utils]
        webhook-url-type = 'subdomain'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: RawConfig = toml::from_str(
            r#"
        port = 7070
        public-url = 'http://localhost:7070/'

        [db]
        name = 'secutils'
        username = 'postgres'
        password = 'password'
        host = 'localhost'
        port = 5432

        [security]
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = false

        [components]
        web-scraper-url = 'http://localhost:7272/'
        search-index-version = 3

        [scheduler]
        web-page-trackers-schedule = '0 * * * * * *'
        web-page-trackers-fetch = '0 * * * * * *'
        notifications-send = '0/30 * * * * * *'

        [subscriptions]
        feature-overview-url = 'http://localhost:7272/'

        [subscriptions.basic.webhooks]
        responders = 1
        responder-requests = 11
        js-runtime-heap-size = 10
        js-runtime-script-execution-time = 20

        [subscriptions.basic.web-scraping]
        trackers = 1
        tracker-revisions = 11

        [subscriptions.basic.web-security]
        policies = 10
        import-policy-from-url = false

        [subscriptions.basic.certificates]
        private-keys = 1
        templates = 11
        private-key-algorithms = ['RSA-1024']

        [subscriptions.standard.webhooks]
        responders = 2
        responder-requests = 22
        js-runtime-heap-size = 30
        js-runtime-script-execution-time = 40

        [subscriptions.standard.web-scraping]
        trackers = 2
        tracker-revisions = 22

        [subscriptions.standard.web-security]
        policies = 1000
        import-policy-from-url = true

        [subscriptions.standard.certificates]
        private-keys = 2
        templates = 22
        private-key-algorithms = ['RSA-2048']

        [subscriptions.professional.webhooks]
        responders = 3
        responder-requests = 33
        js-runtime-heap-size = 50
        js-runtime-script-execution-time = 60

        [subscriptions.professional.web-scraping]
        trackers = 3
        tracker-revisions = 33

        [subscriptions.professional.web-security]
        policies = 1000
        import-policy-from-url = true

        [subscriptions.professional.certificates]
        private-keys = 3
        templates = 33
        private-key-algorithms = ['RSA-4096']

        [subscriptions.ultimate.webhooks]
        responders = 4
        responder-requests = 44
        js-runtime-heap-size = 70
        js-runtime-script-execution-time = 80

        [subscriptions.ultimate.web-scraping]
        trackers = 4
        tracker-revisions = 44

        [subscriptions.ultimate.web-security]
        policies = 1000
        import-policy-from-url = true

        [subscriptions.ultimate.certificates]
        private-keys = 4
        templates = 44

        [utils]
        webhook-url-type = 'subdomain'
    "#,
        )
        .unwrap();

        assert_debug_snapshot!(config, @r###"
        RawConfig {
            port: 7070,
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
                password: Some(
                    "password",
                ),
            },
            security: SecurityConfig {
                session_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                use_insecure_session_cookie: false,
                builtin_users: None,
            },
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
                        responders: 1,
                        responder_requests: 11,
                        js_runtime_heap_size: 10,
                        js_runtime_script_execution_time: 20ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 1,
                        tracker_revisions: 11,
                        tracker_schedules: None,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 1,
                        templates: 11,
                        private_key_algorithms: Some(
                            {
                                "RSA-1024",
                            },
                        ),
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 10,
                        import_policy_from_url: false,
                    },
                },
                standard: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 2,
                        responder_requests: 22,
                        js_runtime_heap_size: 30,
                        js_runtime_script_execution_time: 40ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 2,
                        tracker_revisions: 22,
                        tracker_schedules: None,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 2,
                        templates: 22,
                        private_key_algorithms: Some(
                            {
                                "RSA-2048",
                            },
                        ),
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                },
                professional: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 3,
                        responder_requests: 33,
                        js_runtime_heap_size: 50,
                        js_runtime_script_execution_time: 60ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 3,
                        tracker_revisions: 33,
                        tracker_schedules: None,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 3,
                        templates: 33,
                        private_key_algorithms: Some(
                            {
                                "RSA-4096",
                            },
                        ),
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                },
                ultimate: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 4,
                        responder_requests: 44,
                        js_runtime_heap_size: 70,
                        js_runtime_script_execution_time: 80ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 4,
                        tracker_revisions: 44,
                        tracker_schedules: None,
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 4,
                        templates: 44,
                        private_key_algorithms: None,
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 1000,
                        import_policy_from_url: true,
                    },
                },
            },
            utils: UtilsConfig {
                webhook_url_type: Subdomain,
            },
            smtp: None,
        }
        "###);
    }
}
