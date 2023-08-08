use crate::users::UserDataNamespace;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum InternalUserDataNamespace {
    AccountActivationToken,
    CredentialsResetToken,
    WebPageResourcesTrackersJobs,
}

impl AsRef<str> for InternalUserDataNamespace {
    fn as_ref(&self) -> &str {
        match self {
            InternalUserDataNamespace::AccountActivationToken => "accountActivationToken",
            InternalUserDataNamespace::CredentialsResetToken => "credentialsResetToken",
            InternalUserDataNamespace::WebPageResourcesTrackersJobs => {
                "webPageResourcesTrackersJobs"
            }
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

        assert_eq!(
            InternalUserDataNamespace::CredentialsResetToken.as_ref(),
            "credentialsResetToken"
        );

        assert_eq!(
            InternalUserDataNamespace::WebPageResourcesTrackersJobs.as_ref(),
            "webPageResourcesTrackersJobs"
        );

        Ok(())
    }
}
