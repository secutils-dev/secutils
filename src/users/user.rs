use crate::{
    security::StoredCredentials,
    users::{UserId, UserSubscription},
};
use serde::{ser::SerializeStruct, Serialize, Serializer};
use time::OffsetDateTime;

/// Serializer that makes sure credentials aren't serialized and exposed to the client. Instead, the
/// serialized struct only indicates what credential types are currently configured for the user.
fn stored_credentials_safe_serialize<S>(value: &StoredCredentials, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut credentials = s.serialize_struct("Credentials", 2)?;
    credentials.serialize_field("password", &value.password_hash.is_some())?;
    credentials.serialize_field("passkey", &value.passkey.is_some())?;
    credentials.end()
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: UserId,
    pub email: String,
    pub handle: String,
    #[serde(serialize_with = "stored_credentials_safe_serialize")]
    pub credentials: StoredCredentials,
    #[serde(with = "time::serde::timestamp")]
    pub created: OffsetDateTime,
    pub activated: bool,
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
        security::StoredCredentials,
        tests::{webauthn::SERIALIZED_PASSKEY, MockUserBuilder},
        users::{SubscriptionTier, UserSubscription},
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user_with_password = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "my-email",
            "my-handle",
            StoredCredentials {
                password_hash: Some("my-pass-hash".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        let user_with_passkey = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "my-email",
            "my-handle",
            StoredCredentials::from_passkey(serde_json::from_str(SERIALIZED_PASSKEY)?),
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activated()
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Professional,
            started_at: OffsetDateTime::from_unix_timestamp(1262340001)?,
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        })
        .build();

        let user_with_password_and_passkey = MockUserBuilder::new(
            uuid!("00000000-0000-0000-0000-000000000001").into(),
            "my-email",
            "my-handle",
            StoredCredentials {
                password_hash: Some("my-pass-hash".to_string()),
                passkey: Some(serde_json::from_str(SERIALIZED_PASSKEY)?),
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activated()
        .build();

        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(user_with_password, @r###"
            {
              "email": "my-email",
              "handle": "my-handle",
              "credentials": {
                "password": true,
                "passkey": false
              },
              "created": 1262340000,
              "activated": false,
              "subscription": {
                "tier": "ultimate",
                "startedAt": 1262340001
              }
            }
            "###);
        });

        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(user_with_passkey, @r###"
            {
              "email": "my-email",
              "handle": "my-handle",
              "credentials": {
                "password": false,
                "passkey": true
              },
              "created": 1262340000,
              "activated": true,
              "subscription": {
                "tier": "professional",
                "startedAt": 1262340001
              }
            }
            "###);
        });

        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(user_with_password_and_passkey, @r###"
            {
              "email": "my-email",
              "handle": "my-handle",
              "credentials": {
                "password": true,
                "passkey": true
              },
              "created": 1262340000,
              "activated": true,
              "subscription": {
                "tier": "ultimate",
                "startedAt": 1262340001
              }
            }
            "###);
        });

        Ok(())
    }
}
