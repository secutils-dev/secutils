use crate::users::{User, UserId};
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUser {
    pub id: i64,
    pub email: String,
    pub handle: String,
    pub password_hash: String,
    pub created: i64,
    pub roles: Option<String>,
    pub activation_code: Option<String>,
}

impl TryFrom<RawUser> for User {
    type Error = anyhow::Error;

    fn try_from(raw_user: RawUser) -> Result<Self, Self::Error> {
        Ok(User {
            id: UserId(raw_user.id),
            email: raw_user.email,
            handle: raw_user.handle,
            password_hash: raw_user.password_hash,
            roles: raw_user
                .roles
                .map(|roles_str| roles_str.split(':').map(|part| part.to_string()).collect())
                .unwrap_or_default(),
            created: OffsetDateTime::from_unix_timestamp(raw_user.created)?,
            activation_code: raw_user.activation_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::primary_db::raw_user::RawUser,
        tests::MockUserBuilder,
        users::{User, UserId},
    };
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_user_without_optional_fields() -> anyhow::Result<()> {
        assert_eq!(
            User::try_from(RawUser {
                id: 1,
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: None,
                activation_code: None,
            })?,
            MockUserBuilder::new(
                UserId(1),
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
            User::try_from(RawUser {
                id: 1,
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin".to_string()),
                activation_code: Some("code".to_string()),
            })?,
            MockUserBuilder::new(
                UserId(1),
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
            User::try_from(RawUser {
                id: 1,
                email: "dev@secutils.dev".to_string(),
                handle: "dev-handle".to_string(),
                password_hash: "password-hash".to_string(),
                // January 1, 2000 11:00:00
                created: 946720800,
                roles: Some("admin:superuser".to_string()),
                activation_code: None,
            })?,
            MockUserBuilder::new(
                UserId(1),
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
        assert!(User::try_from(RawUser {
            id: 1,
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
}
