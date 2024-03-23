use crate::{
    config::Config,
    users::{SubscriptionTier, UserSubscription},
};
use std::time::Duration;

/// The feature describing subscription-dependent properties of the webhooks responders.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WebhooksRespondersFeature {
    /// The maximum number of memory available to the user's responder script.
    pub max_script_memory: usize,
    /// The maximum time that user's responder script can take to execute.
    pub max_script_time: Duration,
}

impl WebhooksRespondersFeature {
    /// Returns webhook responders feature properties for the specified user.
    pub fn new(config: &Config, subscription: UserSubscription) -> Self {
        let (max_script_memory, max_script_time) = match subscription.effective_tier() {
            SubscriptionTier::Basic => (5 * 1024 * 1024, Duration::from_secs(5)),
            SubscriptionTier::Standard
            | SubscriptionTier::Professional
            | SubscriptionTier::Ultimate => (
                config.js_runtime.max_heap_size,
                config.js_runtime.max_user_script_execution_time,
            ),
        };

        Self {
            max_script_memory,
            max_script_time,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        tests::mock_config,
        users::{
            user_subscription::subscription_features::WebhooksRespondersFeature, SubscriptionTier,
            UserSubscription,
        },
    };
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[test]
    fn can_get_webhooks_responders_feature() -> anyhow::Result<()> {
        let config = mock_config()?;
        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };
        let features = WebhooksRespondersFeature::new(&config, subscription);
        assert_eq!(features.max_script_memory, 5 * 1024 * 1024);
        assert_eq!(features.max_script_time, Duration::from_secs(5));

        let subscriptions = [
            UserSubscription {
                tier: SubscriptionTier::Standard,
                ..subscription
            },
            UserSubscription {
                tier: SubscriptionTier::Professional,
                ..subscription
            },
            UserSubscription {
                tier: SubscriptionTier::Ultimate,
                ..subscription
            },
        ];

        for subscription in subscriptions {
            let features = WebhooksRespondersFeature::new(&config, subscription);
            assert_eq!(features.max_script_memory, config.js_runtime.max_heap_size);
            assert_eq!(
                features.max_script_time,
                config.js_runtime.max_user_script_execution_time
            );
        }

        Ok(())
    }

    #[test]
    fn can_get_webhooks_responders_feature_considering_trial() -> anyhow::Result<()> {
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
            UserSubscription {
                tier: SubscriptionTier::Ultimate,
                ..subscription
            },
        ];

        for subscription in subscriptions {
            let features = WebhooksRespondersFeature::new(&config, subscription);
            assert_eq!(features.max_script_memory, config.js_runtime.max_heap_size);
            assert_eq!(
                features.max_script_time,
                config.js_runtime.max_user_script_execution_time
            );
        }

        Ok(())
    }
}
