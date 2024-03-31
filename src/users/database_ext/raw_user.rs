use crate::users::{User, UserSubscription};
use anyhow::Context;
use std::borrow::Cow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUser<'s> {
    pub id: Uuid,
    pub email: Cow<'s, str>,
    pub handle: Cow<'s, str>,
    pub credentials: Vec<u8>,
    pub created: OffsetDateTime,
    pub activated: bool,
    pub subscription_tier: i32,
    pub subscription_started_at: OffsetDateTime,
    pub subscription_ends_at: Option<OffsetDateTime>,
    pub subscription_trial_started_at: Option<OffsetDateTime>,
    pub subscription_trial_ends_at: Option<OffsetDateTime>,
}

impl<'u> TryFrom<RawUser<'u>> for User {
    type Error = anyhow::Error;

    fn try_from(raw_user: RawUser) -> Result<Self, Self::Error> {
        Ok(User {
            id: raw_user.id.into(),
            email: raw_user.email.into_owned(),
            handle: raw_user.handle.into_owned(),
            credentials: serde_json::from_slice(raw_user.credentials.as_slice())
                .with_context(|| "Cannot deserialize user credentials".to_string())?,
            created: raw_user.created,
            activated: raw_user.activated,
            subscription: UserSubscription {
                tier: u8::try_from(raw_user.subscription_tier)?.try_into()?,
                started_at: raw_user.subscription_started_at,
                ends_at: raw_user.subscription_ends_at,
                trial_started_at: raw_user.subscription_trial_started_at,
                trial_ends_at: raw_user.subscription_trial_ends_at,
            },
        })
    }
}

impl<'u> TryFrom<&'u User> for RawUser<'u> {
    type Error = anyhow::Error;

    fn try_from(user: &'u User) -> Result<Self, Self::Error> {
        Ok(Self {
            id: *user.id,
            email: Cow::Borrowed(user.email.as_ref()),
            handle: Cow::Borrowed(user.handle.as_ref()),
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
    use super::RawUser;
    use crate::{
        security::StoredCredentials,
        tests::MockUserBuilder,
        users::{SubscriptionTier, User, UserSubscription},
    };
    use insta::assert_debug_snapshot;
    use std::borrow::Cow;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_raw_user_into_user() -> anyhow::Result<()> {
        assert_debug_snapshot!(User::try_from(RawUser {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            email: Cow::Borrowed("dev@secutils.dev"),
            handle: Cow::Borrowed("dev-handle"),
            credentials: serde_json::to_vec(&StoredCredentials { 
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            }).unwrap(),
            // January 1, 2000 11:00:00
            created: OffsetDateTime::from_unix_timestamp(946720800)?,
            activated: true,
            subscription_tier: SubscriptionTier::Ultimate as i32,
            // January 1, 2000 11:00:01
            subscription_started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            subscription_ends_at: None,
            subscription_trial_started_at: None,
            subscription_trial_ends_at: None,
        })?, @r###"
        User {
            id: UserId(
                00000000-0000-0000-0000-000000000001,
            ),
            email: "dev@secutils.dev",
            handle: "dev-handle",
            credentials: StoredCredentials {
                password_hash: Some(
                    "password-hash",
                ),
                passkey: None,
            },
            created: 2000-01-01 10:00:00.0 +00:00:00,
            activated: true,
            subscription: UserSubscription {
                tier: Ultimate,
                started_at: 2000-01-01 10:00:01.0 +00:00:00,
                ends_at: None,
                trial_started_at: None,
                trial_ends_at: None,
            },
        }
        "###);

        assert_debug_snapshot!(User::try_from(RawUser {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            email: Cow::Borrowed("dev@secutils.dev"),
            handle: Cow::Borrowed("dev-handle"),
            credentials: serde_json::to_vec(&StoredCredentials { 
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            }).unwrap(),
            // January 1, 2000 11:00:00
            created: OffsetDateTime::from_unix_timestamp(946720800)?,
            activated: true,
            subscription_tier: SubscriptionTier::Professional as i32,
            // January 1, 2000 11:00:01
            subscription_started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            subscription_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720802)?),
            subscription_trial_started_at: Some(OffsetDateTime::from_unix_timestamp(946720803)?),
            subscription_trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720804)?),
        })?, @r###"
        User {
            id: UserId(
                00000000-0000-0000-0000-000000000001,
            ),
            email: "dev@secutils.dev",
            handle: "dev-handle",
            credentials: StoredCredentials {
                password_hash: Some(
                    "password-hash",
                ),
                passkey: None,
            },
            created: 2000-01-01 10:00:00.0 +00:00:00,
            activated: true,
            subscription: UserSubscription {
                tier: Professional,
                started_at: 2000-01-01 10:00:01.0 +00:00:00,
                ends_at: Some(
                    2000-01-01 10:00:02.0 +00:00:00,
                ),
                trial_started_at: Some(
                    2000-01-01 10:00:03.0 +00:00:00,
                ),
                trial_ends_at: Some(
                    2000-01-01 10:00:04.0 +00:00:00,
                ),
            },
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_convert_user_into_raw_user() -> anyhow::Result<()> {
        assert_eq!(
            RawUser::try_from(
                &MockUserBuilder::new(
                    uuid!("00000000-0000-0000-0000-000000000001").into(),
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
            RawUser {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                email: Cow::Borrowed("dev@secutils.dev"),
                handle: Cow::Borrowed("dev-handle"),
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
    fn can_convert_user_into_raw_user_with_custom_subscription() -> anyhow::Result<()> {
        assert_eq!(
            RawUser::try_from(
                &MockUserBuilder::new(
                    uuid!("00000000-0000-0000-0000-000000000001").into(),
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
            RawUser {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                email: Cow::Borrowed("dev@secutils.dev"),
                handle: Cow::Borrowed("dev-handle"),
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

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(User::try_from(RawUser {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            email: Cow::Borrowed("dev@secutils.dev"),
            handle: Cow::Borrowed("dev-handle"),
            credentials: vec![1, 2, 3],
            created: time::Date::MIN.midnight().assume_utc(),
            activated: true,
            subscription_tier: SubscriptionTier::Ultimate as i32,
            subscription_started_at: time::Date::MIN.midnight().assume_utc(),
            subscription_ends_at: None,
            subscription_trial_started_at: None,
            subscription_trial_ends_at: None,
        })
        .is_err());

        Ok(())
    }
}
