use crate::{security::StoredCredentials, users::SubscriptionTier};
use anyhow::bail;

#[derive(Debug, Clone)]
pub struct BuiltinUser {
    pub email: String,
    pub handle: String,
    pub credentials: StoredCredentials,
    pub tier: SubscriptionTier,
}

impl TryFrom<&str> for BuiltinUser {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let user_properties = value.split(':').collect::<Vec<_>>();
        if user_properties.len() < 4 || user_properties.len() > 5 {
            bail!("Builtin user is malformed.");
        }

        let user_email = user_properties[0].trim();
        let user_password = user_properties[1].trim();
        let user_handle = user_properties[2].trim();
        if user_password.is_empty() || user_email.is_empty() || user_handle.is_empty() {
            bail!(
                "Builtin user cannot have empty password, username, handle, or subscription tier."
            );
        }

        Ok(BuiltinUser {
            email: user_email.to_string(),
            handle: user_handle.to_string(),
            credentials: StoredCredentials::try_from_password(user_password)?,
            tier: user_properties[3].parse::<u8>()?.try_into()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::users::{builtin_user::BuiltinUser, SubscriptionTier};

    #[test]
    fn can_parse_builtin_user() -> anyhow::Result<()> {
        let parsed_user = BuiltinUser::try_from("su@secutils.dev:password:su_handle:100")?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(parsed_user.handle, "su_handle");
        assert_eq!(parsed_user.tier, SubscriptionTier::Ultimate);
        assert!(parsed_user
            .credentials
            .password_hash
            .unwrap()
            .starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));

        let parsed_user = BuiltinUser::try_from("su@secutils.dev:password:su_handle:10")?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(parsed_user.handle, "su_handle");
        assert_eq!(parsed_user.tier, SubscriptionTier::Basic);
        assert!(parsed_user
            .credentials
            .password_hash
            .unwrap()
            .starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));

        Ok(())
    }

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(BuiltinUser::try_from("su@secutils.dev:").is_err());
        assert!(BuiltinUser::try_from("su@secutils.dev").is_err());
        assert!(BuiltinUser::try_from("su@secutils.dev:handle").is_err());
        assert!(BuiltinUser::try_from("su@secutils.dev:handle:").is_err());

        Ok(())
    }
}
