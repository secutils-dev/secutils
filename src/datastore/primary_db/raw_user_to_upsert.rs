use crate::users::User;
use itertools::Itertools;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserToUpsert<'a> {
    pub email: &'a str,
    pub handle: &'a str,
    pub password_hash: &'a str,
    pub created: i64,
    pub roles: Option<String>,
    pub activation_code: Option<&'a str>,
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
            password_hash: user.password_hash.as_ref(),
            created: user.created.unix_timestamp(),
            roles: raw_roles,
            activation_code: user.activation_code.as_deref(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::primary_db::raw_user_to_upsert::RawUserToUpsert, tests::MockUserBuilder,
        users::UserId,
    };
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_raw_user_to_upsert_without_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            RawUserToUpsert::try_from(
                &MockUserBuilder::new(
                    UserId(1),
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    "password-hash".to_string(),
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .build()
            )?,
            RawUserToUpsert {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                password_hash: "password-hash",
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: None,
                activation_code: None,
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
                    "password-hash".to_string(),
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .add_role("admin")
                .set_activation_code("code")
                .build()
            )?,
            RawUserToUpsert {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                password_hash: "password-hash",
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin".to_string()),
                activation_code: Some("code"),
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
                    "password-hash".to_string(),
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .add_role("admin")
                .add_role("superuser")
                .build()
            )?,
            RawUserToUpsert {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                password_hash: "password-hash",
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin:superuser".to_string()),
                activation_code: None,
            }
        );

        Ok(())
    }
}
