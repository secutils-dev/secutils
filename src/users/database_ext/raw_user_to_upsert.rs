use crate::users::User;
use anyhow::Context;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserToUpsert<'a> {
    pub email: &'a str,
    pub handle: &'a str,
    pub credentials: Vec<u8>,
    pub created: OffsetDateTime,
    pub activated: bool,
    pub subscription_tier: i32,
    pub subscription_started_at: OffsetDateTime,
    pub subscription_ends_at: Option<OffsetDateTime>,
    pub subscription_trial_started_at: Option<OffsetDateTime>,
    pub subscription_trial_ends_at: Option<OffsetDateTime>,
}

impl<'a> TryFrom<&'a User> for RawUserToUpsert<'a> {
    type Error = anyhow::Error;

    fn try_from(user: &'a User) -> Result<Self, Self::Error> {
        Ok(Self {
            email: user.email.as_ref(),
            handle: user.handle.as_ref(),
            credentials: serde_json::ser::to_vec(&user.credentials).with_context(|| {
                format!("Failed to serialize user credentials ({}).", user.handle)
            })?,
            created: user.created,
            activated: user.activated,
            subscription_tier: user.subscription.tier as i32,
            subscription_started_at: user.subscription.started_at,
            subscription_ends_at: user.subscription.ends_at,
            subscription_trial_started_at: user.subscription.trial_started_at,
            subscription_trial_ends_at: user.subscription.trial_ends_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUserToUpsert;
    use crate::{
        security::StoredCredentials,
        tests::MockUserBuilder,
        users::{SubscriptionTier, UserSubscription},
    };
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_raw_user_to_upsert() -> anyhow::Result<()> {
        assert_eq!(
            RawUserToUpsert::try_from(
                &MockUserBuilder::new(
                    1.try_into()?,
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    StoredCredentials {
                        password_hash: Some("password-hash".to_string()),
                        ..Default::default()
                    },
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .build()
            )?,
            RawUserToUpsert {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: serde_json::to_vec(&StoredCredentials {
                    password_hash: Some("password-hash".to_string()),
                    ..Default::default()
                })
                .unwrap(),
                // January 1, 2000 11:00:00
                created: OffsetDateTime::from_unix_timestamp(946720800)?,
                subscription_tier: 100,
                // January 1, 2000 11:00:01
                subscription_started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
                subscription_ends_at: None,
                subscription_trial_started_at: None,
                subscription_trial_ends_at: None,
                activated: false,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_to_upsert_with_custom_subscription() -> anyhow::Result<()> {
        assert_eq!(
            RawUserToUpsert::try_from(
                &MockUserBuilder::new(
                    1.try_into()?,
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    StoredCredentials {
                        password_hash: Some("password-hash".to_string()),
                        ..Default::default()
                    },
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .set_subscription(UserSubscription {
                    tier: SubscriptionTier::Standard,
                    started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
                    ends_at: Some(OffsetDateTime::from_unix_timestamp(946720802)?),
                    trial_started_at: Some(OffsetDateTime::from_unix_timestamp(946720803)?),
                    trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720804)?),
                })
                .build()
            )?,
            RawUserToUpsert {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: serde_json::to_vec(&StoredCredentials {
                    password_hash: Some("password-hash".to_string()),
                    ..Default::default()
                })
                .unwrap(),
                // January 1, 2000 11:00:00
                created: OffsetDateTime::from_unix_timestamp(946720800)?,
                activated: false,
                subscription_tier: 20,
                // January 1, 2000 11:00:01
                subscription_started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
                subscription_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720802)?),
                subscription_trial_started_at: Some(OffsetDateTime::from_unix_timestamp(
                    946720803
                )?),
                subscription_trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720804)?),
            }
        );

        Ok(())
    }
}
