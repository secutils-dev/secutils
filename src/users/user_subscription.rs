use crate::config::Config;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, TimestampSeconds};
use time::{Duration, OffsetDateTime};

mod subscription_features;
mod subscription_tier;

pub use self::{subscription_features::SubscriptionFeatures, subscription_tier::SubscriptionTier};

/// The subscription status of a user.
#[serde_as]
#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserSubscription {
    pub tier: SubscriptionTier,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub started_at: OffsetDateTime,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<OffsetDateTime>,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trial_started_at: Option<OffsetDateTime>,
    #[serde_as(as = "Option<TimestampSeconds<i64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trial_ends_at: Option<OffsetDateTime>,
}

impl UserSubscription {
    pub const TRIAL_LENGTH: Duration = Duration::days(14);

    /// Based on the original subscription tier and trial status, returns the effective subscription tier.
    pub fn effective_tier(&self) -> SubscriptionTier {
        if matches!(self.tier, SubscriptionTier::Ultimate) {
            return self.tier;
        }

        match self.trial_ends_at {
            Some(trial_end) if trial_end >= OffsetDateTime::now_utc() => {
                SubscriptionTier::Professional
            }
            _ => self.tier,
        }
    }

    /// Returns all features available for the specified subscription with the specified config.
    pub fn get_features(&self, config: &Config) -> SubscriptionFeatures {
        SubscriptionFeatures::new(config, *self)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        tests::mock_config,
        users::{SubscriptionFeatures, SubscriptionTier, UserSubscription},
    };
    use insta::assert_json_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[test]
    fn trial_length() {
        assert_eq!(
            UserSubscription::TRIAL_LENGTH,
            Duration::from_secs(14 * 24 * 60 * 60)
        );
    }

    #[test]
    fn get_features() -> anyhow::Result<()> {
        let config = mock_config()?;
        let subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            started_at: OffsetDateTime::now_utc(),
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };
        let features = subscription.get_features(&config);
        assert_eq!(features, SubscriptionFeatures::new(&config, subscription));

        Ok(())
    }

    #[test]
    fn can_get_effective_tier() -> anyhow::Result<()> {
        let trial_tiers = [
            SubscriptionTier::Basic,
            SubscriptionTier::Standard,
            SubscriptionTier::Professional,
        ];
        for tier in trial_tiers {
            let subscription = UserSubscription {
                tier,
                started_at: OffsetDateTime::now_utc(),
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            };
            assert_eq!(subscription.effective_tier(), tier);

            let subscription = UserSubscription {
                tier,
                started_at: OffsetDateTime::now_utc(),
                ends_at: None,
                trial_started_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(1))),
                trial_ends_at: Some(OffsetDateTime::now_utc().add(UserSubscription::TRIAL_LENGTH)),
            };
            assert_eq!(
                subscription.effective_tier(),
                SubscriptionTier::Professional
            );

            let subscription = UserSubscription {
                tier,
                started_at: OffsetDateTime::now_utc(),
                ends_at: None,
                trial_started_at: Some(
                    OffsetDateTime::now_utc().sub(UserSubscription::TRIAL_LENGTH),
                ),
                trial_ends_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(1))),
            };
            assert_eq!(subscription.effective_tier(), tier);
        }

        let subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            started_at: OffsetDateTime::now_utc(),
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };
        assert_eq!(subscription.effective_tier(), SubscriptionTier::Ultimate);

        let subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            started_at: OffsetDateTime::now_utc(),
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(1))),
            trial_ends_at: Some(OffsetDateTime::now_utc().add(UserSubscription::TRIAL_LENGTH)),
        };
        assert_eq!(subscription.effective_tier(), SubscriptionTier::Ultimate);

        let subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            started_at: OffsetDateTime::now_utc(),
            ends_at: None,
            trial_started_at: Some(OffsetDateTime::now_utc().sub(UserSubscription::TRIAL_LENGTH)),
            trial_ends_at: Some(OffsetDateTime::now_utc().sub(Duration::from_secs(1))),
        };
        assert_eq!(subscription.effective_tier(), SubscriptionTier::Ultimate);

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let subscription = UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: Some(OffsetDateTime::from_unix_timestamp(1262340001)?),
            trial_started_at: Some(OffsetDateTime::from_unix_timestamp(1262340002)?),
            trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(1262340003)?),
        };
        assert_json_snapshot!(subscription, @r###"
        {
          "tier": "basic",
          "startedAt": 1262340000,
          "endsAt": 1262340001,
          "trialStartedAt": 1262340002,
          "trialEndsAt": 1262340003
        }
        "###);

        let subscription = UserSubscription {
            tier: SubscriptionTier::Ultimate,
            started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        };
        assert_json_snapshot!(subscription, @r###"
        {
          "tier": "ultimate",
          "startedAt": 1262340000
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UserSubscription>(
                r#"
        {
          "tier": "basic",
          "startedAt": 1262340000,
          "endsAt": 1262340001,
          "trialStartedAt": 1262340002,
          "trialEndsAt": 1262340003
        }"#
            )?,
            UserSubscription {
                tier: SubscriptionTier::Basic,
                started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
                ends_at: Some(OffsetDateTime::from_unix_timestamp(1262340001)?),
                trial_started_at: Some(OffsetDateTime::from_unix_timestamp(1262340002)?),
                trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(1262340003)?),
            }
        );

        assert_eq!(
            serde_json::from_str::<UserSubscription>(
                r#"
        {
          "tier": "standard",
          "startedAt": 1262340000
        }"#
            )?,
            UserSubscription {
                tier: SubscriptionTier::Standard,
                started_at: OffsetDateTime::from_unix_timestamp(1262340000)?,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            }
        );

        Ok(())
    }
}
