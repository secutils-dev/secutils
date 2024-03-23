use crate::config::{
    utils_config::UtilsConfig, ComponentsConfig, JsRuntimeConfig, SchedulerJobsConfig,
    SecurityConfig, SmtpConfig, SubscriptionsConfig,
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
    /// Security configuration (session, built-in users etc.).
    pub security: SecurityConfig,
    /// Configuration for the components that are deployed separately.
    pub components: ComponentsConfig,
    /// Configuration for the JS runtime.
    pub js_runtime: JsRuntimeConfig,
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
            public_url: Url::parse(&format!("http://localhost:{port}"))
                .expect("Cannot parse public URL parameter."),
            security: SecurityConfig::default(),
            components: ComponentsConfig::default(),
            js_runtime: JsRuntimeConfig::default(),
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

        [security]
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = false

        [components]
        web-scraper-url = 'http://localhost:7272/'
        search-index-version = 3

        [js-runtime]
        max-heap-size = 10485760
        max-user-script-execution-time = 30000

        [scheduler]
        web-page-trackers-schedule = '0 * * * * * *'
        web-page-trackers-fetch = '0 * * * * * *'
        notifications-send = '0/30 * * * * * *'

        [subscriptions]
        feature-overview-url = 'http://localhost:7272/'

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

        [security]
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = false

        [components]
        web-scraper-url = 'http://localhost:7272/'
        search-index-version = 3

        [js-runtime]
        max-heap-size = 10485760
        max-user-script-execution-time = 30000

        [scheduler]
        web-page-trackers-schedule = '0 * * * * * *'
        web-page-trackers-fetch = '0 * * * * * *'
        notifications-send = '0/30 * * * * * *'

        [subscriptions]
        feature-overview-url = 'http://localhost:7272/'

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
            js_runtime: JsRuntimeConfig {
                max_heap_size: 10485760,
                max_user_script_execution_time: 30s,
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
            },
            utils: UtilsConfig {
                webhook_url_type: Subdomain,
            },
            smtp: None,
        }
        "###);
    }
}
