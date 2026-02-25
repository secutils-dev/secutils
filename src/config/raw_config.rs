use crate::config::{
    ComponentsConfig, RetrackConfig, SchedulerJobsConfig, SecurityConfig, SmtpConfig,
    SubscriptionsConfig, database_config::DatabaseConfig, http_config::HttpConfig,
    utils_config::UtilsConfig,
};
use figment::{Figment, Metadata, Profile, Provider, providers, providers::Format, value};
use serde_derive::{Deserialize, Serialize};
use url::Url;

/// Raw configuration structure that is used to read the configuration from the file.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RawConfig {
    /// Defines a TCP port to listen on.
    pub port: u16,
    /// External/public URL through which the service is being accessed.
    pub public_url: Url,
    /// Database configuration.
    pub db: DatabaseConfig,
    /// Security configuration (session, built-in users, etc.).
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
    /// Configuration for the HTTP functionality.
    pub http: HttpConfig,
    /// Configuration for the Retrack service.
    pub retrack: RetrackConfig,
}

impl RawConfig {
    /// Reads the configuration from the file (TOML) and merges it with the default values.
    pub fn read_from_file(path: &str) -> anyhow::Result<Self> {
        Ok(Figment::from(RawConfig::default())
            .merge(providers::Toml::file(path))
            .merge(providers::Env::prefixed("SECUTILS_").split("__"))
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
            http: HttpConfig::default(),
            retrack: RetrackConfig::default(),
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
    use crate::config::RawConfig;
    use insta::{assert_debug_snapshot, assert_toml_snapshot};
    use url::Url;

    #[test]
    fn serialization_and_default() {
        let mut default_config = RawConfig::default();
        default_config.subscriptions.feature_overview_url =
            Some(Url::parse("http://localhost:7272").unwrap());

        assert_toml_snapshot!(default_config, @r###"
        port = 7070
        public_url = 'http://localhost:7070/'

        [db]
        name = 'secutils'
        host = 'localhost'
        port = 5432
        username = 'postgres'

        [security]
        session_cookie_name = 'id'

        [components]
        kratos_url = 'http://localhost:4433/'
        kratos_admin_url = 'http://localhost:4434/'
        search_index_version = 4

        [scheduler]
        notifications_send = '0/30 * * * * *'

        [subscriptions]
        feature_overview_url = 'http://localhost:7272/'
        [subscriptions.basic.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        [subscriptions.basic.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000
        [subscriptions.basic.certificates]
        private_keys = 100
        templates = 1000
        [subscriptions.basic.web_security]
        policies = 1000
        import_policy_from_url = true
        [subscriptions.standard.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        [subscriptions.standard.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000
        [subscriptions.standard.certificates]
        private_keys = 100
        templates = 1000
        [subscriptions.standard.web_security]
        policies = 1000
        import_policy_from_url = true
        [subscriptions.professional.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        [subscriptions.professional.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000
        [subscriptions.professional.certificates]
        private_keys = 100
        templates = 1000
        [subscriptions.professional.web_security]
        policies = 1000
        import_policy_from_url = true
        [subscriptions.ultimate.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        [subscriptions.ultimate.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000
        [subscriptions.ultimate.certificates]
        private_keys = 100
        templates = 1000
        [subscriptions.ultimate.web_security]
        policies = 1000
        import_policy_from_url = true

        [utils]
        webhook_url_type = 'subdomain'
        diff_context_radius = 3
        [http.client]
        timeout = 30000
        pool_idle_timeout = 5000
        max_retries = 3
        verbose = false

        [retrack]
        host = 'http://localhost:7676/'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: RawConfig = toml::from_str(
            r#"
        port = 7070
        public_url = 'http://localhost:7070/'

        [db]
        name = 'secutils'
        username = 'postgres'
        password = 'password'
        host = 'localhost'
        port = 5432

        [http.client]
        timeout = 60000
        pool_idle_timeout = 6000
        max_retries = 6
        verbose = true

        [retrack]
        host = 'http://localhost:7777/'

        [security]
        session_cookie_name = 'id2'

        [security.preconfigured_users."dev@secutils.dev"]
        handle = "dev"
        tier = "ultimate"

        [components]
        kratos_url = 'http://localhost:4433/'
        kratos_admin_url = 'http://localhost:4434/'
        search_index_version = 3

        [scheduler]
        notifications_send = '0/30 * * * * * *'

        [subscriptions]
        feature_overview_url = 'http://localhost:7272/'

        [subscriptions.basic.webhooks]
        responders = 1
        responder_requests = 11
        responder_custom_subdomain_prefix = false
        js_runtime_heap_size = 10
        js_runtime_script_execution_time = 20

        [subscriptions.basic.web_scraping]
        trackers = 1
        tracker_revisions = 11
        min_schedule_interval = 10_000

        [subscriptions.basic.web_security]
        policies = 10
        import_policy_from_url = false

        [subscriptions.basic.certificates]
        private_keys = 1
        templates = 11
        private_key_algorithms = ['RSA-1024']

        [subscriptions.standard.webhooks]
        responders = 2
        responder_requests = 22
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 30
        js_runtime_script_execution_time = 40

        [subscriptions.standard.web_scraping]
        trackers = 2
        tracker_revisions = 22
        min_schedule_interval = 20_000

        [subscriptions.standard.web_security]
        policies = 1000
        import_policy_from_url = true

        [subscriptions.standard.certificates]
        private_keys = 2
        templates = 22
        private_key_algorithms = ['RSA-2048']

        [subscriptions.professional.webhooks]
        responders = 3
        responder_requests = 33
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 50
        js_runtime_script_execution_time = 60

        [subscriptions.professional.web_scraping]
        trackers = 3
        tracker_revisions = 33
        min_schedule_interval = 30_000

        [subscriptions.professional.web_security]
        policies = 1000
        import_policy_from_url = true

        [subscriptions.professional.certificates]
        private_keys = 3
        templates = 33
        private_key_algorithms = ['RSA-4096']

        [subscriptions.ultimate.webhooks]
        responders = 4
        responder_requests = 44
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 70
        js_runtime_script_execution_time = 80

        [subscriptions.ultimate.web_scraping]
        trackers = 4
        tracker_revisions = 44
        min_schedule_interval = 40_000

        [subscriptions.ultimate.web_security]
        policies = 1000
        import_policy_from_url = true

        [subscriptions.ultimate.certificates]
        private_keys = 4
        templates = 44

        [utils]
        webhook_url_type = 'subdomain'
        diff_context_radius = 3
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
                session_cookie_name: "id2",
                jwt_secret: None,
                operators: None,
                preconfigured_users: Some(
                    {
                        "dev@secutils.dev": PreconfiguredUserConfig {
                            handle: "dev",
                            tier: Ultimate,
                        },
                    },
                ),
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
                search_index_version: 3,
            },
            scheduler: SchedulerJobsConfig {
                notifications_send: "0/30 * * * * * *",
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
                        responder_custom_subdomain_prefix: false,
                        js_runtime_heap_size: 10,
                        js_runtime_script_execution_time: 20ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 1,
                        tracker_revisions: 11,
                        tracker_schedules: None,
                        min_schedule_interval: 10s,
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
                        responder_custom_subdomain_prefix: true,
                        js_runtime_heap_size: 30,
                        js_runtime_script_execution_time: 40ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 2,
                        tracker_revisions: 22,
                        tracker_schedules: None,
                        min_schedule_interval: 20s,
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
                        responder_custom_subdomain_prefix: true,
                        js_runtime_heap_size: 50,
                        js_runtime_script_execution_time: 60ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 3,
                        tracker_revisions: 33,
                        tracker_schedules: None,
                        min_schedule_interval: 30s,
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
                        responder_custom_subdomain_prefix: true,
                        js_runtime_heap_size: 70,
                        js_runtime_script_execution_time: 80ms,
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 4,
                        tracker_revisions: 44,
                        tracker_schedules: None,
                        min_schedule_interval: 40s,
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
                diff_context_radius: 3,
            },
            smtp: None,
            http: HttpConfig {
                client: HttpClientConfig {
                    timeout: 60s,
                    pool_idle_timeout: 6s,
                    max_retries: 6,
                    verbose: true,
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
                        7777,
                    ),
                    path: "/",
                    query: None,
                    fragment: None,
                },
            },
        }
        "###);
    }
}
