use crate::api::{Api, UsersApi};
use anyhow::bail;
use std::collections::HashSet;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BuiltinUser {
    pub email: String,
    pub password_hash: String,
    pub roles: HashSet<String>,
}

impl TryFrom<&str> for BuiltinUser {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let user_properties = value.split(':').collect::<Vec<_>>();
        if user_properties.len() < 2 || user_properties.len() > 3 {
            bail!("Builtin user is malformed.");
        }

        let user_email = user_properties[0].trim();
        let user_password = user_properties[1].trim();
        if user_password.is_empty() || user_email.is_empty() {
            bail!("Builtin user cannot have empty password or username.");
        }

        Ok(BuiltinUser {
            email: user_email.to_string(),
            password_hash: UsersApi::generate_user_password_hash(user_password)?,
            roles: if user_properties.len() == 3 {
                user_properties[2]
                    .split(',')
                    .filter_map(|role_str| {
                        let role_str = role_str.trim();
                        if role_str.is_empty() {
                            None
                        } else {
                            Some(role_str.to_lowercase())
                        }
                    })
                    .collect::<HashSet<_>>()
            } else {
                HashSet::with_capacity(0)
            },
        })
    }
}

pub fn initialize_builtin_users<BU: AsRef<str>>(
    api: &Api,
    builtin_users: BU,
) -> anyhow::Result<()> {
    log::info!("Initializing builtin users");
    let users = api.users();

    let mut initialized_builtin_users = 0;
    for builtin_user_str in builtin_users.as_ref().split('|') {
        users.upsert_builtin(BuiltinUser::try_from(builtin_user_str)?)?;
        initialized_builtin_users += 1;
    }

    log::info!(
        "Successfully initialized {} builtin users.",
        initialized_builtin_users
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::users::builtin_user::BuiltinUser;
    use std::collections::HashSet;

    #[test]
    fn can_parse_builtin_user_with_multiple_roles() -> anyhow::Result<()> {
        let parsed_user = BuiltinUser::try_from("su@secutils.dev:password:one,Two")?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(
            parsed_user.roles,
            ["one", "two"]
                .into_iter()
                .map(|role| role.to_string())
                .collect()
        );
        assert_eq!(
            parsed_user
                .password_hash
                .starts_with("$argon2id$v=19$m=4096,t=3,p=1$"),
            true
        );

        Ok(())
    }

    #[test]
    fn can_parse_builtin_user_with_single_roles() -> anyhow::Result<()> {
        let parsed_user = BuiltinUser::try_from("su@secutils.dev:password:one")?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(
            parsed_user.roles,
            ["one"].into_iter().map(|role| role.to_string()).collect()
        );
        assert_eq!(
            parsed_user
                .password_hash
                .starts_with("$argon2id$v=19$m=4096,t=3,p=1$"),
            true
        );

        Ok(())
    }

    #[test]
    fn can_parse_builtin_user_without_roles() -> anyhow::Result<()> {
        let parsed_user = BuiltinUser::try_from("su@secutils.dev:password:")?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(parsed_user.roles, HashSet::new());
        assert_eq!(
            parsed_user
                .password_hash
                .starts_with("$argon2id$v=19$m=4096,t=3,p=1$"),
            true
        );

        let parsed_user = BuiltinUser::try_from("su@secutils.dev:password")?;
        assert_eq!(parsed_user.email, "su@secutils.dev");
        assert_eq!(parsed_user.roles, HashSet::new());
        assert_eq!(
            parsed_user
                .password_hash
                .starts_with("$argon2id$v=19$m=4096,t=3,p=1$"),
            true
        );

        Ok(())
    }

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert_eq!(BuiltinUser::try_from("su@secutils.dev:").is_err(), true);
        assert_eq!(BuiltinUser::try_from("su@secutils.dev").is_err(), true);

        Ok(())
    }
}
