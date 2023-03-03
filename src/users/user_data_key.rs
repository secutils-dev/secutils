use crate::users::{InternalUserDataNamespace, PublicUserDataNamespace, UserDataNamespace};
use std::convert::From;

/// Defines a composite (namespace + key) user data key.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct UserDataKey<'p> {
    pub namespace: UserDataNamespace,
    pub key: Option<&'p str>,
}

/// Base implementation for the user data with the default key.
impl<'p> From<UserDataNamespace> for UserDataKey<'p> {
    fn from(namespace: UserDataNamespace) -> Self {
        Self {
            namespace,
            key: None,
        }
    }
}

/// Base implementation for the public user data with the default key.
impl<'p> From<PublicUserDataNamespace> for UserDataKey<'p> {
    fn from(namespace: PublicUserDataNamespace) -> Self {
        UserDataNamespace::from(namespace).into()
    }
}

/// Base implementation for the internal user data with the default key.
impl<'p> From<InternalUserDataNamespace> for UserDataKey<'p> {
    fn from(namespace: InternalUserDataNamespace) -> Self {
        UserDataNamespace::from(namespace).into()
    }
}

/// Implementation for the user data with the non-default data key.
impl<'p> From<(UserDataNamespace, &'p str)> for UserDataKey<'p> {
    fn from((namespace, key): (UserDataNamespace, &'p str)) -> Self {
        Self {
            namespace,
            key: Some(key),
        }
    }
}

/// Implementation for the public user data with the non-default data key.
impl<'p> From<(PublicUserDataNamespace, &'p str)> for UserDataKey<'p> {
    fn from((namespace, key): (PublicUserDataNamespace, &'p str)) -> Self {
        (UserDataNamespace::from(namespace), key).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::users::{
        InternalUserDataNamespace, PublicUserDataNamespace, UserDataKey, UserDataNamespace,
    };

    #[test]
    fn properly_converted_from_different_types() -> anyhow::Result<()> {
        assert_eq!(
            UserDataKey::from(PublicUserDataNamespace::UserSettings),
            UserDataKey {
                namespace: UserDataNamespace::Public(PublicUserDataNamespace::UserSettings),
                key: None
            }
        );
        assert_eq!(
            UserDataKey::from(InternalUserDataNamespace::AccountActivationToken),
            UserDataKey {
                namespace: UserDataNamespace::Internal(
                    InternalUserDataNamespace::AccountActivationToken
                ),
                key: None
            }
        );
        assert_eq!(
            UserDataKey::from((PublicUserDataNamespace::AutoResponders, "my-responder")),
            UserDataKey {
                namespace: UserDataNamespace::Public(PublicUserDataNamespace::AutoResponders),
                key: Some("my-responder")
            }
        );

        Ok(())
    }
}
