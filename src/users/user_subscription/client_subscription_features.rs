use crate::users::SubscriptionFeatures;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ClientSubscriptionCertificatesConfig<'sf> {
    /// The list of allowed private key algorithms for a particular subscription.
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key_algorithms: &'sf Option<HashSet<String>>,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ClientSubscriptionWebhooksConfig {
    /// The number of responders requests per responder that retained for a particular subscription.
    responder_requests: usize,
    /// Indicates whether the subscription supports custom subdomain prefix for responders.
    responder_custom_subdomain_prefix: bool,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ClientSubscriptionWebScrapingConfig<'sf> {
    /// The number of tracker revisions per tracker that retained for a particular subscription.
    tracker_revisions: usize,
    /// The list of allowed schedules for the trackers for a particular subscription.
    #[serde(skip_serializing_if = "Option::is_none")]
    tracker_schedules: &'sf Option<HashSet<String>>,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ClientSubscriptionWebSecurityConfig {
    /// Indicates whether it's allowed to import policies from a URL for a particular subscription.
    import_policy_from_url: bool,
}

/// Subscription feature representation meant to be consumed by the client. The `SubscriptionConfig`
/// is meant to be only used for TOML config. However, it's not suitable for JSON, and we need to
/// use a separate type.
#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientSubscriptionFeatures<'sf> {
    /// The config managing the certificates utilities for a particular subscription.
    certificates: ClientSubscriptionCertificatesConfig<'sf>,
    /// The config managing the webhooks utilities for a particular subscription.
    webhooks: ClientSubscriptionWebhooksConfig,
    /// The config managing the web scraping utilities for a particular subscription.
    web_scraping: ClientSubscriptionWebScrapingConfig<'sf>,
    /// The config managing the web security utilities for a particular subscription.
    web_security: ClientSubscriptionWebSecurityConfig,
}

impl<'sf> From<SubscriptionFeatures<'sf>> for ClientSubscriptionFeatures<'sf> {
    fn from(value: SubscriptionFeatures<'sf>) -> Self {
        Self {
            certificates: ClientSubscriptionCertificatesConfig {
                private_key_algorithms: &value.config.certificates.private_key_algorithms,
            },
            webhooks: ClientSubscriptionWebhooksConfig {
                responder_requests: value.config.webhooks.responder_requests,
                responder_custom_subdomain_prefix: value
                    .config
                    .webhooks
                    .responder_custom_subdomain_prefix,
            },
            web_scraping: ClientSubscriptionWebScrapingConfig {
                tracker_revisions: value.config.web_scraping.tracker_revisions,
                tracker_schedules: &value.config.web_scraping.tracker_schedules,
            },
            web_security: ClientSubscriptionWebSecurityConfig {
                import_policy_from_url: value.config.web_security.import_policy_from_url,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        config::{
            SubscriptionCertificatesConfig, SubscriptionConfig, SubscriptionWebScrapingConfig,
            SubscriptionWebSecurityConfig, SubscriptionWebhooksConfig,
        },
        tests::mock_config,
        users::{
            ClientSubscriptionFeatures, SubscriptionTier, UserSubscription,
            user_subscription::subscription_features::SubscriptionFeatures,
        },
        utils::certificates::{PrivateKeyAlgorithm, PrivateKeySize},
    };
    use insta::assert_json_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.subscriptions.basic = SubscriptionConfig {
            webhooks: SubscriptionWebhooksConfig {
                responders: 1,
                responder_requests: 11,
                responder_custom_subdomain_prefix: false,
                js_runtime_heap_size: 2,
                js_runtime_script_execution_time: Duration::from_secs(3),
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
            secrets: Default::default(),
        };

        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(60 * 60))),
            trial_ends_at: Some(OffsetDateTime::now_utc().add(Duration::from_secs(60 * 60))),
        };

        let features =
            ClientSubscriptionFeatures::from(SubscriptionFeatures::new(&config, subscription));
        assert_json_snapshot!(features, @r###"
        {
          "certificates": {},
          "webhooks": {
            "responderRequests": 30,
            "responderCustomSubdomainPrefix": true
          },
          "webScraping": {
            "trackerRevisions": 30
          },
          "webSecurity": {
            "importPolicyFromUrl": true
          }
        }
        "###);

        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };

        let features =
            ClientSubscriptionFeatures::from(SubscriptionFeatures::new(&config, subscription));
        assert_json_snapshot!(features, { ".webScraping.trackerSchedules" => insta::sorted_redaction() }, @r###"
        {
          "certificates": {
            "privateKeyAlgorithms": [
              "RSA-1024"
            ]
          },
          "webhooks": {
            "responderRequests": 11,
            "responderCustomSubdomainPrefix": false
          },
          "webScraping": {
            "trackerRevisions": 11,
            "trackerSchedules": [
              "@",
              "@daily",
              "@monthly",
              "@weekly"
            ]
          },
          "webSecurity": {
            "importPolicyFromUrl": false
          }
        }
        "###);

        let features = ClientSubscriptionFeatures::from(SubscriptionFeatures::new(
            &config,
            UserSubscription {
                tier: SubscriptionTier::Ultimate,
                ..subscription
            },
        ));

        assert_json_snapshot!(features, @r###"
        {
          "certificates": {},
          "webhooks": {
            "responderRequests": 30,
            "responderCustomSubdomainPrefix": true
          },
          "webScraping": {
            "trackerRevisions": 30
          },
          "webSecurity": {
            "importPolicyFromUrl": true
          }
        }
        "###);

        Ok(())
    }
}
