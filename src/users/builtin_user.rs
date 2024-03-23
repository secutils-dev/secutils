use crate::{config::BuiltinUserConfig, security::StoredCredentials, users::SubscriptionTier};
use anyhow::bail;

#[derive(Debug, Clone)]
pub struct BuiltinUser {
    /// Builtin user email.
    pub email: String,
    /// Builtin user handle (used to construct unique user sub-domain).
    pub handle: String,
    /// Builtin user credentials.
    pub credentials: StoredCredentials,
    /// Builtin user subscription tier.
    pub tier: SubscriptionTier,
}

impl TryFrom<&BuiltinUserConfig> for BuiltinUser {
    type Error = anyhow::Error;

    fn try_from(value: &BuiltinUserConfig) -> Result<Self, Self::Error> {
        if value.password.is_empty() || value.email.is_empty() || value.handle.is_empty() {
            bail!("Builtin user cannot have empty password, username, or handle.");
        }

        Ok(BuiltinUser {
            email: value.email.to_owned(),
            handle: value.handle.to_owned(),
            credentials: StoredCredentials::try_from_password(&value.password)?,
            tier: value.tier,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::users::{
        builtin_user::{BuiltinUser, BuiltinUserConfig},
        SubscriptionTier,
    };

    #[test]
    fn can_parse_builtin_user() -> anyhow::Result<()> {
        let user_config = BuiltinUserConfig {
            email: "su@secutils.dev".to_string(),
            handle: "su_handle".to_string(),
            password: "password".to_string(),
            tier: SubscriptionTier::Ultimate,
        };
        let parsed_user = BuiltinUser::try_from(&user_config)?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(parsed_user.handle, "su_handle");
        assert_eq!(parsed_user.tier, SubscriptionTier::Ultimate);
        assert!(parsed_user
            .credentials
            .password_hash
            .unwrap()
            .starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));

        let user_config = BuiltinUserConfig {
            email: "su@secutils.dev".to_string(),
            handle: "su_handle".to_string(),
            password: "password".to_string(),
            tier: SubscriptionTier::Basic,
        };
        let parsed_user = BuiltinUser::try_from(&user_config)?;
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
        let user_config = BuiltinUserConfig {
            email: "su@secutils.dev".to_string(),
            handle: "su_handle".to_string(),
            password: "".to_string(),
            tier: SubscriptionTier::Basic,
        };
        assert!(BuiltinUser::try_from(&user_config).is_err());

        let user_config = BuiltinUserConfig {
            email: "".to_string(),
            handle: "su_handle".to_string(),
            password: "password".to_string(),
            tier: SubscriptionTier::Basic,
        };
        assert!(BuiltinUser::try_from(&user_config).is_err());

        let user_config = BuiltinUserConfig {
            email: "su@secutils.dev".to_string(),
            handle: "".to_string(),
            password: "password".to_string(),
            tier: SubscriptionTier::Basic,
        };
        assert!(BuiltinUser::try_from(&user_config).is_err());

        Ok(())
    }
}
