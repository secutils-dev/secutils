use crate::{authentication::StoredCredentials, users::UserId};
use serde::{ser::SerializeStruct, Serializer};
use serde_derive::Serialize;
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

/// Serializer that makes sure expected activation code isn't serialized and exposed to the client.
/// Instead, we produce a boolean indicating whether user has their account or not.
fn activation_code_safe_serialize<S>(value: &Option<String>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_bool(value.is_none())
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
    #[serde(
        rename = "activated",
        serialize_with = "activation_code_safe_serialize"
    )]
    pub activation_code: Option<String>,
}

impl AsRef<User> for User {
    fn as_ref(&self) -> &User {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::StoredCredentials,
        tests::{webauthn::SERIALIZED_PASSKEY, MockUserBuilder},
        users::UserId,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user_with_password = MockUserBuilder::new(
            UserId(1),
            "my-email",
            "my-handle",
            StoredCredentials {
                password_hash: Some("my-pass-hash".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activation_code("some-code")
        .add_role("ADMIN")
        .build();

        let user_with_passkey = MockUserBuilder::new(
            UserId(1),
            "my-email",
            "my-handle",
            StoredCredentials::from_passkey(serde_json::from_str(SERIALIZED_PASSKEY)?),
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        let user_with_password_and_passkey = MockUserBuilder::new(
            UserId(1),
            "my-email",
            "my-handle",
            StoredCredentials {
                password_hash: Some("my-pass-hash".to_string()),
                passkey: Some(serde_json::from_str(SERIALIZED_PASSKEY)?),
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
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
