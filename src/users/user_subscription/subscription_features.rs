mod webhooks_responders;

pub use self::webhooks_responders::WebhooksRespondersFeature;
use crate::{
    config::Config,
    users::{SubscriptionTier, UserSubscription},
};
use serde::Serialize;

/// The subscription-dependent features available to the user.
#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionFeatures {
    /// Indicates whether the user has access to the administrative functionality..
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub admin: bool,
    /// The subscription-dependent features of the webhooks responders.
    #[serde(skip_serializing)]
    pub webhooks_responders: WebhooksRespondersFeature,
}

impl SubscriptionFeatures {
    /// Returns all features available for the specified user subscription.
    pub fn new(config: &Config, subscription: UserSubscription) -> Self {
        Self {
            admin: matches!(subscription.tier, SubscriptionTier::Ultimate),
            webhooks_responders: WebhooksRespondersFeature::new(config, subscription),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        tests::mock_config,
        users::{
            user_subscription::subscription_features::SubscriptionFeatures, SubscriptionTier,
            UserSubscription,
        },
    };
    use insta::assert_json_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[test]
    fn can_get_subscription_features() -> anyhow::Result<()> {
        let config = mock_config()?;
        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(60 * 60))),
            trial_ends_at: Some(OffsetDateTime::now_utc().add(Duration::from_secs(60 * 60))),
        };

        let subscriptions = [
            subscription,
            UserSubscription {
                tier: SubscriptionTier::Standard,
                ..subscription
            },
            UserSubscription {
                tier: SubscriptionTier::Professional,
                ..subscription
            },
        ];

        for subscription in subscriptions {
            let features = SubscriptionFeatures::new(&config, subscription);
            assert!(!features.admin);
            assert_eq!(
                features.webhooks_responders.max_script_memory,
                config.js_runtime.max_heap_size
            );
            assert_eq!(
                features.webhooks_responders.max_script_time,
                config.js_runtime.max_user_script_execution_time
            );
        }

        let ultimate_subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            ..subscription
        };

        let features = SubscriptionFeatures::new(&config, ultimate_subscription);
        assert!(features.admin);
        assert_eq!(
            features.webhooks_responders.max_script_memory,
            config.js_runtime.max_heap_size
        );
        assert_eq!(
            features.webhooks_responders.max_script_time,
            config.js_runtime.max_user_script_execution_time
        );

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
