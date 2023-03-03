use crate::users::UserDataNamespace;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum InternalUserDataNamespace {
    AccountActivationToken,
}

impl AsRef<str> for InternalUserDataNamespace {
    fn as_ref(&self) -> &str {
        match self {
            InternalUserDataNamespace::AccountActivationToken => "accountActivationToken",
        }
    }
}

impl From<InternalUserDataNamespace> for UserDataNamespace {
    fn from(value: InternalUserDataNamespace) -> Self {
        UserDataNamespace::Internal(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::users::InternalUserDataNamespace;

    #[test]
    fn proper_str_reference() -> anyhow::Result<()> {
        assert_eq!(
            InternalUserDataNamespace::AccountActivationToken.as_ref(),
            "accountActivationToken"
        );

        Ok(())
    }
}
