use crate::users::{InternalUserDataType, PublicUserDataType, UserDataType};
use anyhow::Context;
use std::convert::From;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct UserDataKey<'p> {
    data_type: UserDataType,
    data_path: Option<&'p str>,
}

/// Base implementation for the user data without additional data path.
impl<'p> From<UserDataType> for UserDataKey<'p> {
    fn from(data_type: UserDataType) -> Self {
        Self {
            data_type,
            data_path: None,
        }
    }
}

/// Base implementation for the public user data without additional data path.
impl<'p> From<PublicUserDataType> for UserDataKey<'p> {
    fn from(data_type: PublicUserDataType) -> Self {
        UserDataType::from(data_type).into()
    }
}

/// Base implementation for the internal user data without additional data path.
impl<'p> From<InternalUserDataType> for UserDataKey<'p> {
    fn from(data_type: InternalUserDataType) -> Self {
        UserDataType::from(data_type).into()
    }
}

/// Implementation for the user data with additional data path.
impl<'p> From<(UserDataType, &'p str)> for UserDataKey<'p> {
    fn from((data_type, data_path): (UserDataType, &'p str)) -> Self {
        Self {
            data_type,
            data_path: Some(data_path),
        }
    }
}

/// Implementation for the public user data with additional data path.
impl<'p> From<(PublicUserDataType, &'p str)> for UserDataKey<'p> {
    fn from((data_type, data_path): (PublicUserDataType, &'p str)) -> Self {
        (UserDataType::from(data_type), data_path).into()
    }
}

/// Implementation for the internal user data with additional data path.
impl<'p> From<(InternalUserDataType, &'p str)> for UserDataKey<'p> {
    fn from((data_type, data_path): (InternalUserDataType, &'p str)) -> Self {
        (UserDataType::from(data_type), data_path).into()
    }
}

/// This implementation produces a string representation of the data key stored in the database.
impl<'p> TryFrom<UserDataKey<'p>> for String {
    type Error = anyhow::Error;

    fn try_from(user_data_key: UserDataKey<'p>) -> Result<Self, Self::Error> {
        serde_json::to_string(&user_data_key.data_type)
            .map(|data_type_segment| {
                if let Some(path) = user_data_key.data_path {
                    format!(r###"{}__"{}""###, data_type_segment, path)
                } else {
                    data_type_segment
                }
            })
            .with_context(|| "Cannot serialize user data type.")
    }
}

#[cfg(test)]
mod tests {
    use crate::{datastore::UserDataKey, users::PublicUserDataType};

    #[test]
    fn properly_converted_to_string() -> anyhow::Result<()> {
        assert_eq!(
            String::try_from(UserDataKey::from(PublicUserDataType::AutoResponders)).unwrap(),
            r###""autoResponders""###
        );
        assert_eq!(
            String::try_from(UserDataKey::from((
                PublicUserDataType::AutoResponders,
                "my-responder"
            )))
            .unwrap(),
            r###""autoResponders"__"my-responder""###
        );

        Ok(())
    }
}
