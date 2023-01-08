use crate::users::User;
use itertools::Itertools;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUser {
    pub email: String,
    pub handle: String,
    pub password_hash: String,
    pub created: i64,
    pub roles: Option<String>,
    pub activation_code: Option<String>,
}

impl TryInto<User> for RawUser {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<User, Self::Error> {
        Ok(User {
            email: self.email,
            handle: self.handle,
            password_hash: self.password_hash,
            roles: self
                .roles
                .map(|roles_str| roles_str.split(':').map(|part| part.to_string()).collect())
                .unwrap_or_default(),
            created: OffsetDateTime::from_unix_timestamp(self.created)?,
            activation_code: self.activation_code,
        })
    }
}

impl TryInto<RawUser> for User {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<RawUser, Self::Error> {
        let raw_roles = if !self.roles.is_empty() {
            Some(self.roles.iter().sorted().join(":"))
        } else {
            None
        };

        Ok(RawUser {
            email: self.email,
            handle: self.handle,
            password_hash: self.password_hash,
            created: self.created.unix_timestamp(),
            roles: raw_roles,
            activation_code: self.activation_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{datastore::primary_db::raw_user::RawUser, tests::MockUserBuilder, users::User};
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_user_without_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            TryInto::<User>::try_into(RawUser {
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: None,
                activation_code: None,
            })?,
            MockUserBuilder::new(
                "dev@secutils.dev".to_string(),
                "dev-handle".to_string(),
                "password-hash".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build()
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_user_with_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            TryInto::<User>::try_into(RawUser {
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin".to_string()),
                activation_code: Some("code".to_string()),
            })?,
            MockUserBuilder::new(
                "dev@secutils.dev".to_string(),
                "dev-handle".to_string(),
                "password-hash".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .add_role("admin")
            .set_activation_code("code")
            .build()
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_user_with_multiple_roles() -> anyhow::Result<()> {
        assert_eq!(
            TryInto::<User>::try_into(RawUser {
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin:superuser".to_string()),
                activation_code: None,
            })?,
            MockUserBuilder::new(
                "dev@secutils.dev".to_string(),
                "dev-handle".to_string(),
                "password-hash".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .add_role("admin")
            .add_role("superuser")
            .build()
        );

        Ok(())
    }

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(TryInto::<User>::try_into(RawUser {
            email: "dev@secutils.dev".to_string(),
            handle: "dev-handle".to_string(),
            password_hash: "password-hash".to_string(),
            created: time::Date::MIN.midnight().assume_utc().unix_timestamp() - 1,
            roles: None,
            activation_code: None,
        })
        .is_err());

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_without_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            TryInto::<RawUser>::try_into(
                MockUserBuilder::new(
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    "password-hash".to_string(),
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .build()
            )?,
            RawUser {
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: None,
                activation_code: None,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_with_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            TryInto::<RawUser>::try_into(
                MockUserBuilder::new(
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    "password-hash".to_string(),
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .add_role("admin")
                .set_activation_code("code")
                .build()
            )?,
            RawUser {
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin".to_string()),
                activation_code: Some("code".to_string()),
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_with_multiple_roles() -> anyhow::Result<()> {
        assert_eq!(
            TryInto::<RawUser>::try_into(
                MockUserBuilder::new(
                    "dev@secutils.dev".to_string(),
                    "dev-handle".to_string(),
                    "password-hash".to_string(),
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .add_role("admin")
                .add_role("superuser")
                .build()
            )?,
            RawUser {
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin:superuser".to_string()),
                activation_code: None,
            }
        );

        Ok(())
    }
}
