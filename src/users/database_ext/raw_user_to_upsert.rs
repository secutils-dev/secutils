use crate::users::User;
use anyhow::Context;
use itertools::Itertools;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserToUpsert<'a> {
    pub email: &'a str,
    pub handle: &'a str,
    pub credentials: Vec<u8>,
    pub created: i64,
    pub roles: Option<String>,
    pub activated: i64,
}

impl<'a> TryFrom<&'a User> for RawUserToUpsert<'a> {
    type Error = anyhow::Error;

    fn try_from(user: &'a User) -> Result<Self, Self::Error> {
        let raw_roles = if !user.roles.is_empty() {
            Some(user.roles.iter().sorted().join(":"))
        } else {
            None
        };

        Ok(Self {
            email: user.email.as_ref(),
            handle: user.handle.as_ref(),
            credentials: serde_json::ser::to_vec(&user.credentials).with_context(|| {
                format!("Failed to serialize user credentials ({}).", user.handle)
            })?,
            created: user.created.unix_timestamp(),
            roles: raw_roles,
            activated: if user.activated { 1 } else { 0 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUserToUpsert;
    use crate::{security::StoredCredentials, tests::MockUserBuilder, users::UserId};
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_raw_user_to_upsert_without_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            RawUserToUpsert::try_from(
                &MockUserBuilder::new(
                    UserId(1),
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
                created: 946720800,
                roles: None,
                activated: 0,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_to_upsert_with_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            RawUserToUpsert::try_from(
                &MockUserBuilder::new(
                    UserId(1),
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    StoredCredentials {
                        password_hash: Some("password-hash".to_string()),
                        ..Default::default()
                    },
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .add_role("admin")
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
                created: 946720800,
                roles: Some("admin".to_string()),
                activated: 0,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_to_upsert_with_multiple_roles() -> anyhow::Result<()> {
        assert_eq!(
            RawUserToUpsert::try_from(
                &MockUserBuilder::new(
                    UserId(1),
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    StoredCredentials {
                        password_hash: Some("password-hash".to_string()),
                        ..Default::default()
                    },
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .add_role("admin")
                .add_role("superuser")
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
                created: 946720800,
                roles: Some("admin:superuser".to_string()),
                activated: 0,
            }
        );

        Ok(())
    }
}
