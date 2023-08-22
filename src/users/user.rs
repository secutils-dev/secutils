use crate::{security::StoredCredentials, users::UserId};
use serde::{ser::SerializeStruct, Serialize, Serializer};
use std::collections::HashSet;
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

#[derive(Serialize, Debug, Clone)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: UserId,
    pub email: String,
    pub handle: String,
    #[serde(serialize_with = "stored_credentials_safe_serialize")]
    pub credentials: StoredCredentials,
    pub roles: HashSet<String>,
    #[serde(with = "time::serde::timestamp")]
    pub created: OffsetDateTime,
    pub activated: bool,
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
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user_with_password = MockUserBuilder::new(
            1.try_into()?,
            "my-email",
            "my-handle",
            StoredCredentials {
                password_hash: Some("my-pass-hash".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .add_role("ADMIN")
        .build();

        let user_with_passkey = MockUserBuilder::new(
            1.try_into()?,
            "my-email",
            "my-handle",
            StoredCredentials::from_passkey(serde_json::from_str(SERIALIZED_PASSKEY)?),
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activated()
        .build();

        let user_with_password_and_passkey = MockUserBuilder::new(
            1.try_into()?,
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
              "roles": [
                "admin"
              ],
              "created": 1262340000,
              "activated": false
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
              "roles": [],
              "created": 1262340000,
              "activated": true
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
              "roles": [],
              "created": 1262340000,
              "activated": true
            }
            "###);
        });

        Ok(())
    }
}
