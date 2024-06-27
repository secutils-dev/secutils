mod subscription_certificates_config;
mod subscription_config;
mod subscription_web_scraping_config;
mod subscription_web_security_config;
mod subscription_webhooks_config;

use crate::users::SubscriptionTier;
use serde_derive::{Deserialize, Serialize};
use url::Url;

pub use self::{
    subscription_certificates_config::SubscriptionCertificatesConfig,
    subscription_config::SubscriptionConfig,
    subscription_web_scraping_config::SubscriptionWebScrapingConfig,
    subscription_web_security_config::SubscriptionWebSecurityConfig,
    subscription_webhooks_config::SubscriptionWebhooksConfig,
};

/// Configuration related to the Secutils.dev subscriptions.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct SubscriptionsConfig {
    /// The URL to access the subscription management page.
    pub manage_url: Option<Url>,
    /// The URL to access the feature overview page.
    pub feature_overview_url: Option<Url>,
    /// The configuration specific for the basic subscription tier.
    pub basic: SubscriptionConfig,
    /// The configuration specific for the standard subscription tier.
    pub standard: SubscriptionConfig,
    /// The configuration specific for the professional subscription tier.
    pub professional: SubscriptionConfig,
    /// The configuration specific for the ultimate subscription tier.
    pub ultimate: SubscriptionConfig,
}

impl SubscriptionsConfig {
    /// Returns the subscription configuration for the given tier.
    pub fn get_tier_config(&self, tier: SubscriptionTier) -> &SubscriptionConfig {
        match tier {
            SubscriptionTier::Basic => &self.basic,
            SubscriptionTier::Standard => &self.standard,
            SubscriptionTier::Professional => &self.professional,
            SubscriptionTier::Ultimate => &self.ultimate,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{
            SubscriptionCertificatesConfig, SubscriptionConfig, SubscriptionWebScrapingConfig,
            SubscriptionWebSecurityConfig, SubscriptionWebhooksConfig, SubscriptionsConfig,
        },
        users::SubscriptionTier,
        utils::certificates::{PrivateKeyAlgorithm, PrivateKeySize},
    };
    use insta::assert_toml_snapshot;
    use std::time::Duration;
    use url::Url;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SubscriptionsConfig::default(), @r###"
        [basic.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [basic.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [basic.certificates]
        private_keys = 100
        templates = 1000

        [basic.web_security]
        policies = 1000
        import_policy_from_url = true
        [standard.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [standard.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [standard.certificates]
        private_keys = 100
        templates = 1000

        [standard.web_security]
        policies = 1000
        import_policy_from_url = true
        [professional.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [professional.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [professional.certificates]
        private_keys = 100
        templates = 1000

        [professional.web_security]
        policies = 1000
        import_policy_from_url = true
        [ultimate.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [ultimate.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [ultimate.certificates]
        private_keys = 100
        templates = 1000

        [ultimate.web_security]
        policies = 1000
        import_policy_from_url = true
        "###);

        let config = SubscriptionsConfig {
            manage_url: Some(Url::parse("http://localhost:7272").unwrap()),
            feature_overview_url: Some(Url::parse("http://localhost:7272").unwrap()),
            basic: SubscriptionConfig::default(),
            standard: SubscriptionConfig::default(),
            professional: SubscriptionConfig::default(),
            ultimate: SubscriptionConfig::default(),
        };
        assert_toml_snapshot!(config, @r###"
        manage_url = 'http://localhost:7272/'
        feature_overview_url = 'http://localhost:7272/'
        [basic.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [basic.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [basic.certificates]
        private_keys = 100
        templates = 1000

        [basic.web_security]
        policies = 1000
        import_policy_from_url = true
        [standard.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [standard.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [standard.certificates]
        private_keys = 100
        templates = 1000

        [standard.web_security]
        policies = 1000
        import_policy_from_url = true
        [professional.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [professional.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [professional.certificates]
        private_keys = 100
        templates = 1000

        [professional.web_security]
        policies = 1000
        import_policy_from_url = true
        [ultimate.webhooks]
        responders = 100
        responder_requests = 30
        responder_custom_subdomains = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000

        [ultimate.web_scraping]
        trackers = 100
        tracker_revisions = 30
        min_schedule_interval = 10000

        [ultimate.certificates]
        private_keys = 100
        templates = 1000

        [ultimate.web_security]
        policies = 1000
        import_policy_from_url = true
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionsConfig = toml::from_str(
            r#"
        manage_url = 'http://localhost:7272/'
        feature_overview_url = 'http://localhost:7272/'

        [basic.webhooks]
        responders = 1
        responder_requests = 11
        responder_custom_subdomains = false
        js_runtime_heap_size = 10
        js_runtime_script_execution_time = 20

        [basic.web_scraping]
        trackers = 1
        tracker_revisions = 11
        tracker_schedules = ["@", "@daily", "@weekly", "@monthly"]
        min_schedule_interval = 10_000

        [basic.certificates]
        private_keys = 1
        templates = 11
        private_key_algorithms = ['RSA-1024']

        [basic.web_security]
        policies = 10
        import_policy_from_url = false

        [standard.webhooks]
        responders = 2
        responder_requests = 22
        responder_custom_subdomains = true
        js_runtime_heap_size = 30
        js_runtime_script_execution_time = 40

        [standard.web_scraping]
        trackers = 2
        tracker_revisions = 22
        tracker_schedules = ["@", "@hourly", "@daily", "@weekly", "@monthly"]
        min_schedule_interval = 20_000

        [standard.web_security]
        policies = 1000
        import_policy_from_url = true

        [standard.certificates]
        private_keys = 2
        templates = 22
        private_key_algorithms = ['RSA-2048']

        [professional.webhooks]
        responders = 3
        responder_requests = 33
        responder_custom_subdomains = true
        js_runtime_heap_size = 50
        js_runtime_script_execution_time = 60

        [professional.web_scraping]
        trackers = 3
        tracker_revisions = 33
        min_schedule_interval = 30_000

        [professional.web_security]
        policies = 1000
        import_policy_from_url = true

        [professional.certificates]
        private_keys = 3
        templates = 33

        [ultimate.webhooks]
        responders = 4
        responder_requests = 44
        responder_custom_subdomains = true
        js_runtime_heap_size = 70
        js_runtime_script_execution_time = 80

        [ultimate.web_scraping]
        trackers = 4
        tracker_revisions = 44
        min_schedule_interval = 40_000

        [ultimate.web_security]
        policies = 1000
        import_policy_from_url = true

        [ultimate.certificates]
        private_keys = 4
        templates = 44
    "#,
        )
        .unwrap();
        assert_eq!(
            config,
            SubscriptionsConfig {
                manage_url: Some(Url::parse("http://localhost:7272").unwrap()),
                feature_overview_url: Some(Url::parse("http://localhost:7272").unwrap()),
                basic: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 1,
                        responder_requests: 11,
                        responder_custom_subdomains: false,
                        js_runtime_heap_size: 10,
                        js_runtime_script_execution_time: Duration::from_millis(20),
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 1,
                        tracker_revisions: 11,
                        tracker_schedules: Some(
                            [
                                '@'.to_string(),
                                "@daily".to_string(),
                                "@weekly".to_string(),
                                "@monthly".to_string()
                            ]
                            .into_iter()
                            .collect()
                        ),
                        min_schedule_interval: Duration::from_secs(10),
                    },
                    web_security: SubscriptionWebSecurityConfig {
                        policies: 10,
                        import_policy_from_url: false
                    },
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 1,
                        templates: 11,
                        private_key_algorithms: Some(
                            [PrivateKeyAlgorithm::Rsa {
                                key_size: PrivateKeySize::Size1024
                            }
                            .to_string()]
                            .into_iter()
                            .collect()
                        )
                    }
                },
                standard: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 2,
                        responder_requests: 22,
                        responder_custom_subdomains: true,
                        js_runtime_heap_size: 30,
                        js_runtime_script_execution_time: Duration::from_millis(40),
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 2,
                        tracker_revisions: 22,
                        tracker_schedules: Some(
                            [
                                '@'.to_string(),
                                "@hourly".to_string(),
                                "@daily".to_string(),
                                "@weekly".to_string(),
                                "@monthly".to_string()
                            ]
                            .into_iter()
                            .collect()
                        ),
                        min_schedule_interval: Duration::from_secs(20),
                    },
                    web_security: SubscriptionWebSecurityConfig::default(),
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 2,
                        templates: 22,
                        private_key_algorithms: Some(
                            [PrivateKeyAlgorithm::Rsa {
                                key_size: PrivateKeySize::Size2048
                            }
                            .to_string()]
                            .into_iter()
                            .collect()
                        )
                    }
                },
                professional: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 3,
                        responder_requests: 33,
                        responder_custom_subdomains: true,
                        js_runtime_heap_size: 50,
                        js_runtime_script_execution_time: Duration::from_millis(60),
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 3,
                        tracker_revisions: 33,
                        tracker_schedules: None,
                        min_schedule_interval: Duration::from_secs(30),
                    },
                    web_security: SubscriptionWebSecurityConfig::default(),
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 3,
                        templates: 33,
                        private_key_algorithms: None
                    }
                },
                ultimate: SubscriptionConfig {
                    webhooks: SubscriptionWebhooksConfig {
                        responders: 4,
                        responder_requests: 44,
                        responder_custom_subdomains: true,
                        js_runtime_heap_size: 70,
                        js_runtime_script_execution_time: Duration::from_millis(80),
                    },
                    web_scraping: SubscriptionWebScrapingConfig {
                        trackers: 4,
                        tracker_revisions: 44,
                        tracker_schedules: None,
                        min_schedule_interval: Duration::from_secs(40),
                    },
                    web_security: SubscriptionWebSecurityConfig::default(),
                    certificates: SubscriptionCertificatesConfig {
                        private_keys: 4,
                        templates: 44,
                        private_key_algorithms: None
                    }
                },
            }
        );
    }

    #[test]
    fn can_retrieve_tier_config() {
        let config = SubscriptionsConfig {
            manage_url: Some(Url::parse("http://localhost:7272").unwrap()),
            feature_overview_url: Some(Url::parse("http://localhost:7272").unwrap()),
            basic: SubscriptionConfig {
                webhooks: SubscriptionWebhooksConfig {
                    responders: 1,
                    responder_requests: 11,
                    responder_custom_subdomains: false,
                    js_runtime_heap_size: 10,
                    js_runtime_script_execution_time: Duration::from_millis(20),
                },
                web_scraping: SubscriptionWebScrapingConfig {
                    trackers: 1,
                    tracker_revisions: 11,
                    tracker_schedules: Some(
                        [
                            '@'.to_string(),
                            "@daily".to_string(),
                            "@weekly".to_string(),
                            "@monthly".to_string(),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    min_schedule_interval: Duration::from_secs(10),
                },
                web_security: SubscriptionWebSecurityConfig {
                    policies: 10,
                    import_policy_from_url: false,
                },
                certificates: SubscriptionCertificatesConfig {
                    private_keys: 1,
                    templates: 11,
                    private_key_algorithms: Some(
                        [PrivateKeyAlgorithm::Rsa {
                            key_size: PrivateKeySize::Size1024,
                        }
                        .to_string()]
                        .into_iter()
                        .collect(),
                    ),
                },
            },
            standard: SubscriptionConfig {
                webhooks: SubscriptionWebhooksConfig {
                    responders: 2,
                    responder_requests: 22,
                    responder_custom_subdomains: true,
                    js_runtime_heap_size: 30,
                    js_runtime_script_execution_time: Duration::from_millis(40),
                },
                web_scraping: SubscriptionWebScrapingConfig {
                    trackers: 2,
                    tracker_revisions: 22,
                    tracker_schedules: Some(
                        [
                            '@'.to_string(),
                            "@hourly".to_string(),
                            "@daily".to_string(),
                            "@weekly".to_string(),
                            "@monthly".to_string(),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    min_schedule_interval: Duration::from_secs(20),
                },
                web_security: SubscriptionWebSecurityConfig::default(),
                certificates: SubscriptionCertificatesConfig {
                    private_keys: 2,
                    templates: 22,
                    private_key_algorithms: Some(
                        [PrivateKeyAlgorithm::Rsa {
                            key_size: PrivateKeySize::Size2048,
                        }
                        .to_string()]
                        .into_iter()
                        .collect(),
                    ),
                },
            },
            professional: SubscriptionConfig {
                webhooks: SubscriptionWebhooksConfig {
                    responders: 3,
                    responder_requests: 33,
                    responder_custom_subdomains: true,
                    js_runtime_heap_size: 50,
                    js_runtime_script_execution_time: Duration::from_millis(60),
                },
                web_scraping: SubscriptionWebScrapingConfig {
                    trackers: 3,
                    tracker_revisions: 33,
                    tracker_schedules: None,
                    min_schedule_interval: Duration::from_secs(30),
                },
                web_security: SubscriptionWebSecurityConfig::default(),
                certificates: SubscriptionCertificatesConfig {
                    private_keys: 3,
                    templates: 33,
                    private_key_algorithms: None,
                },
            },
            ultimate: SubscriptionConfig {
                webhooks: SubscriptionWebhooksConfig {
                    responders: 4,
                    responder_requests: 44,
                    responder_custom_subdomains: true,
                    js_runtime_heap_size: 70,
                    js_runtime_script_execution_time: Duration::from_millis(80),
                },
                web_scraping: SubscriptionWebScrapingConfig {
                    trackers: 4,
                    tracker_revisions: 44,
                    tracker_schedules: None,
                    min_schedule_interval: Duration::from_secs(40),
                },
                web_security: SubscriptionWebSecurityConfig::default(),
                certificates: SubscriptionCertificatesConfig {
                    private_keys: 4,
                    templates: 44,
                    private_key_algorithms: None,
                },
            },
        };

        assert_eq!(
            config.get_tier_config(SubscriptionTier::Basic),
            &config.basic
        );
        assert_eq!(
            config.get_tier_config(SubscriptionTier::Standard),
            &config.standard
        );
        assert_eq!(
            config.get_tier_config(SubscriptionTier::Professional),
            &config.professional
        );
        assert_eq!(
            config.get_tier_config(SubscriptionTier::Ultimate),
            &config.ultimate
        );
    }
}
