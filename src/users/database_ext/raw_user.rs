use crate::users::{User, UserSubscription};
use anyhow::Context;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUser {
    pub id: i32,
    pub email: String,
    pub handle: String,
    pub credentials: Vec<u8>,
    pub created: OffsetDateTime,
    pub activated: bool,
    pub subscription_tier: i64,
    pub subscription_started_at: OffsetDateTime,
    pub subscription_ends_at: Option<OffsetDateTime>,
    pub subscription_trial_started_at: Option<OffsetDateTime>,
    pub subscription_trial_ends_at: Option<OffsetDateTime>,
}

impl TryFrom<RawUser> for User {
    type Error = anyhow::Error;

    fn try_from(raw_user: RawUser) -> Result<Self, Self::Error> {
        Ok(User {
            id: raw_user.id.try_into()?,
            email: raw_user.email,
            handle: raw_user.handle,
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

#[cfg(test)]
mod tests {
    use super::RawUser;
    use crate::{
        security::StoredCredentials,
        users::{SubscriptionTier, User},
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_user() -> anyhow::Result<()> {
        assert_debug_snapshot!(User::try_from(RawUser {
            id: 1,
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            credentials: serde_json::to_vec(&StoredCredentials { 
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            }).unwrap(),
            // January 1, 2000 11:00:00
            created: OffsetDateTime::from_unix_timestamp(946720800)?,
            activated: true,
            subscription_tier: SubscriptionTier::Ultimate as i64,
            // January 1, 2000 11:00:01
            subscription_started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            subscription_ends_at: None,
            subscription_trial_started_at: None,
            subscription_trial_ends_at: None,
        })?, @r###"
        User {
            id: UserId(
                1,
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
            id: 1,
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            credentials: serde_json::to_vec(&StoredCredentials { 
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            }).unwrap(),
            // January 1, 2000 11:00:00
            created: OffsetDateTime::from_unix_timestamp(946720800)?,
            activated: true,
            subscription_tier: SubscriptionTier::Professional as i64,
            // January 1, 2000 11:00:01
            subscription_started_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            subscription_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720802)?),
            subscription_trial_started_at: Some(OffsetDateTime::from_unix_timestamp(946720803)?),
            subscription_trial_ends_at: Some(OffsetDateTime::from_unix_timestamp(946720804)?),
        })?, @r###"
        User {
            id: UserId(
                1,
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
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(User::try_from(RawUser {
            id: 1,
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            credentials: vec![1, 2, 3],
            created: time::Date::MIN.midnight().assume_utc(),
            activated: true,
            subscription_tier: SubscriptionTier::Ultimate as i64,
            subscription_started_at: time::Date::MIN.midnight().assume_utc(),
            subscription_ends_at: None,
            subscription_trial_started_at: None,
            subscription_trial_ends_at: None,
        })
        .is_err());

        Ok(())
    }
}
