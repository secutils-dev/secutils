use crate::users::UserDataNamespace;
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

/// Implementation for the user data with the non-default data key.
impl<'p> From<(UserDataNamespace, &'p str)> for UserDataKey<'p> {
    fn from((namespace, key): (UserDataNamespace, &'p str)) -> Self {
        Self {
            namespace,
            key: Some(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::users::{UserDataKey, UserDataNamespace};

    #[test]
    fn properly_converted_from_different_types() -> anyhow::Result<()> {
        assert_eq!(
            UserDataKey::from(UserDataNamespace::UserSettings),
            UserDataKey {
                namespace: UserDataNamespace::UserSettings,
                key: None
            }
        );

        Ok(())
    }
}
