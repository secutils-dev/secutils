use crate::{authentication::StoredCredentials, users::UserId};
use serde_derive::Serialize;
use std::collections::HashSet;
use time::OffsetDateTime;

#[derive(Serialize, Debug, Clone)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: UserId,
    pub email: String,
    pub handle: String,
    #[serde(skip_serializing)]
    pub credentials: StoredCredentials,
    pub roles: HashSet<String>,
    #[serde(with = "time::serde::timestamp")]
    pub created: OffsetDateTime,
    #[serde(skip_serializing)]
    pub activation_code: Option<String>,
}

impl AsRef<User> for User {
    fn as_ref(&self) -> &User {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{authentication::StoredCredentials, tests::MockUserBuilder, users::UserId};
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
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

        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(user, @r###"
            {
              "email": "my-email",
              "handle": "my-handle",
              "roles": [
                "admin"
              ],
              "created": 1262340000
            }
            "###);
        });

        Ok(())
    }
}
