use crate::{
    config::{Config, SubscriptionConfig},
    users::{SubscriptionTier, UserSubscription},
};
use serde::Serialize;

/// The subscription-dependent features available to the user.
#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionFeatures<'c> {
    /// Indicates whether the user has access to the administrative functionality..
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub admin: bool,
    /// The subscription-dependent config.
    #[serde(skip_serializing)]
    pub config: &'c SubscriptionConfig,
}

impl<'c> SubscriptionFeatures<'c> {
    /// Returns all features available for the specified user subscription.
    pub fn new(config: &'c Config, subscription: UserSubscription) -> Self {
        Self {
            admin: matches!(subscription.tier, SubscriptionTier::Ultimate),
            config: config
                .subscriptions
                .get_tier_config(subscription.effective_tier()),
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
            user_subscription::subscription_features::SubscriptionFeatures, SubscriptionTier,
            UserSubscription,
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
    fn can_get_subscription_features() -> anyhow::Result<()> {
        let mut config = mock_config()?;

        config.subscriptions.basic = SubscriptionConfig {
            webhooks: SubscriptionWebhooksConfig {
                responders: 1,
                responder_requests: 11,
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
        };

        config.subscriptions.standard = SubscriptionConfig {
            webhooks: SubscriptionWebhooksConfig {
                responders: 2,
                responder_requests: 22,
                js_runtime_heap_size: 3,
                js_runtime_script_execution_time: Duration::from_secs(4),
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
        };

        config.subscriptions.professional = SubscriptionConfig {
            webhooks: SubscriptionWebhooksConfig {
                responders: 3,
                responder_requests: 33,
                js_runtime_heap_size: 4,
                js_runtime_script_execution_time: Duration::from_secs(5),
            },
            web_scraping: SubscriptionWebScrapingConfig {
                trackers: 3,
                tracker_revisions: 33,
                tracker_schedules: None,
            },
            web_security: SubscriptionWebSecurityConfig::default(),
            certificates: SubscriptionCertificatesConfig {
                private_keys: 3,
                templates: 33,
                private_key_algorithms: None,
            },
        };

        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };

        let features = SubscriptionFeatures::new(&config, subscription);
        assert!(!features.admin);
        assert_eq!(features.config, &config.subscriptions.basic);

        let subscription = UserSubscription {
            tier: SubscriptionTier::Standard,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };

        let features = SubscriptionFeatures::new(&config, subscription);
        assert!(!features.admin);
        assert_eq!(features.config, &config.subscriptions.standard);

        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(60 * 60))),
            trial_ends_at: Some(OffsetDateTime::now_utc().add(Duration::from_secs(60 * 60))),
        };

        let features = SubscriptionFeatures::new(&config, subscription);
        assert!(!features.admin);
        assert_eq!(features.config, &config.subscriptions.professional);

        let features = SubscriptionFeatures::new(
            &config,
            UserSubscription {
                tier: SubscriptionTier::Standard,
                ..subscription
            },
        );
        assert!(!features.admin);
        assert_eq!(features.config, &config.subscriptions.professional);

        let features = SubscriptionFeatures::new(
            &config,
            UserSubscription {
                tier: SubscriptionTier::Professional,
                ..subscription
            },
        );
        assert!(!features.admin);
        assert_eq!(features.config, &config.subscriptions.professional);

        let ultimate_subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            ..subscription
        };

        let features = SubscriptionFeatures::new(&config, ultimate_subscription);
        assert!(features.admin);
        assert_eq!(features.config, &config.subscriptions.ultimate);

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let config = mock_config()?;
        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(60 * 60))),
            trial_ends_at: Some(OffsetDateTime::now_utc().add(Duration::from_secs(60 * 60))),
        };

        let features = SubscriptionFeatures::new(&config, subscription);
        assert_json_snapshot!(features, @"{}");

        let features = SubscriptionFeatures::new(
            &config,
            UserSubscription {
                tier: SubscriptionTier::Ultimate,
                ..subscription
            },
        );
        assert_json_snapshot!(features, @r###"
        {
          "admin": true
        }
        "###);

        Ok(())
    }
}
