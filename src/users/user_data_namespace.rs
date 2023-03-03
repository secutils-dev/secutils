use crate::users::{InternalUserDataNamespace, PublicUserDataNamespace};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum UserDataNamespace {
    Public(PublicUserDataNamespace),
    Internal(InternalUserDataNamespace),
}

impl AsRef<str> for UserDataNamespace {
    fn as_ref(&self) -> &str {
        match self {
            UserDataNamespace::Public(namespace) => namespace.as_ref(),
            UserDataNamespace::Internal(namespace) => namespace.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::users::{InternalUserDataNamespace, PublicUserDataNamespace, UserDataNamespace};

    #[test]
    fn proper_str_reference() -> anyhow::Result<()> {
        assert_eq!(
            UserDataNamespace::Internal(InternalUserDataNamespace::AccountActivationToken).as_ref(),
            "accountActivationToken"
        );

        assert_eq!(
            UserDataNamespace::Public(PublicUserDataNamespace::UserSettings).as_ref(),
            "userSettings"
        );

        Ok(())
    }
}
