use crate::users::{UserId, UserSubscription};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing)]
    pub id: UserId,
    pub email: String,
    pub handle: String,
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    pub is_activated: bool,
    /// Indicates whether the user has access to the operator functionality.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_operator: bool,
    pub subscription: UserSubscription,
}

impl AsRef<User> for User {
    fn as_ref(&self) -> &User {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockUserBuilder,
        users::{SubscriptionTier, UserSubscription},
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "my-email",
            "my-handle",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        let user_with_subscription = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "my-email",
            "my-handle",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_is_activated()
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Professional,
            started_at: OffsetDateTime::from_unix_timestamp(1262340001)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        })
        .build();

        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(user, @r###"
            {
              "email": "my-email",
              "handle": "my-handle",
              "createdAt": 1262340000,
              "isActivated": false,
              "subscription": {
                "tier": "ultimate",
                "startedAt": 1262340001
              }
            }
            "###);
        });

        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(user_with_subscription, @r###"
            {
              "email": "my-email",
              "handle": "my-handle",
              "createdAt": 1262340000,
              "isActivated": true,
              "subscription": {
                "tier": "professional",
                "startedAt": 1262340001
              }
            }
            "###);
        });

        Ok(())
    }
}
