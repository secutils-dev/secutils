use serde::{Deserialize, Serialize};

/// Represents the user subscription tier.
#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum SubscriptionTier {
    Basic = 10,
    Standard = 20,
    Professional = 30,
    Ultimate = 100,
}

impl TryFrom<u8> for SubscriptionTier {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            10 => Ok(SubscriptionTier::Basic),
            20 => Ok(SubscriptionTier::Standard),
            30 => Ok(SubscriptionTier::Professional),
            100 => Ok(SubscriptionTier::Ultimate),
            value => Err(anyhow::anyhow!("Invalid user tier value {value}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::users::SubscriptionTier;

    #[test]
    fn can_parse_subscription_tier() -> anyhow::Result<()> {
        assert_eq!(SubscriptionTier::try_from(10)?, SubscriptionTier::Basic);
        assert_eq!(SubscriptionTier::try_from(20)?, SubscriptionTier::Standard);
        assert_eq!(
            SubscriptionTier::try_from(30)?,
            SubscriptionTier::Professional
        );
        assert_eq!(SubscriptionTier::try_from(100)?, SubscriptionTier::Ultimate);
        for value in [0, 5, 15, 25, 35, 50, 75, 90, 95, 99, 101, 200] {
            assert!(SubscriptionTier::try_from(value).is_err());
        }

        Ok(())
    }

    #[test]
    fn can_convert_tier_to_number() {
        assert_eq!(SubscriptionTier::Basic as u8, 10);
        assert_eq!(SubscriptionTier::Standard as u8, 20);
        assert_eq!(SubscriptionTier::Professional as u8, 30);
        assert_eq!(SubscriptionTier::Ultimate as u8, 100);
    }
}
