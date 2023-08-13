use crate::users::{User, UserId};
use anyhow::Context;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUser {
    pub id: i64,
    pub email: String,
    pub handle: String,
    pub credentials: Vec<u8>,
    pub created: i64,
    pub roles: Option<String>,
    pub activated: i64,
}

impl TryFrom<RawUser> for User {
    type Error = anyhow::Error;

    fn try_from(raw_user: RawUser) -> Result<Self, Self::Error> {
        Ok(User {
            id: UserId(raw_user.id),
            email: raw_user.email,
            handle: raw_user.handle,
            credentials: serde_json::from_slice(raw_user.credentials.as_slice())
                .with_context(|| "Cannot deserialize user credentials".to_string())?,
            roles: raw_user
                .roles
                .map(|roles_str| roles_str.split(':').map(|part| part.to_string()).collect())
                .unwrap_or_default(),
            created: OffsetDateTime::from_unix_timestamp(raw_user.created)?,
            activated: raw_user.activated > 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUser;
    use crate::{security::StoredCredentials, users::User};
    use insta::assert_debug_snapshot;

    #[test]
    fn can_convert_into_user_without_optional_fields() -> anyhow::Result<()> {
        assert_debug_snapshot!(User::try_from(RawUser {
            id: 1,
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            credentials: serde_json::to_vec(&StoredCredentials { 
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            }).unwrap(),
            // January 1, 2000 11:00:00
            created: 946720800,
            roles: None,
            activated: 1,
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
            roles: {},
            created: 2000-01-01 10:00:00.0 +00:00:00,
            activated: true,
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_convert_into_user_with_optional_fields() -> anyhow::Result<()> {
        assert_debug_snapshot!(User::try_from(RawUser {
            id: 1,
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            credentials: serde_json::to_vec(&StoredCredentials { 
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            }).unwrap(),
            // January 1, 2000 11:00:00
            created: 946720800,
            roles: Some("admin".to_string()),
            activated: 0,
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
            roles: {
                "admin",
            },
            created: 2000-01-01 10:00:00.0 +00:00:00,
            activated: false,
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_convert_into_user_with_multiple_roles() -> anyhow::Result<()> {
        assert_eq!(
            User::try_from(RawUser {
                id: 1,
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                credentials: serde_json::to_vec(&StoredCredentials {
                    password_hash: Some("password-hash".to_string()),
                    ..Default::default()
                })
                .unwrap(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin:superuser".to_string()),
                activated: 1,
            })?
            .roles,
            ["admin".to_string(), "superuser".to_string()]
                .into_iter()
                .collect()
        );

        Ok(())
    }

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(User::try_from(RawUser {
            id: 1,
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            credentials: serde_json::to_vec(&StoredCredentials {
                password_hash: Some("password-hash".to_string()),
                ..Default::default()
            })
            .unwrap(),
            created: time::Date::MIN.midnight().assume_utc().unix_timestamp() - 1,
            roles: None,
            activated: 1,
        })
        .is_err());

        Ok(())
    }
}
