use crate::users::UserDataNamespace;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PublicUserDataNamespace {
    UserSettings,
}

impl AsRef<str> for PublicUserDataNamespace {
    fn as_ref(&self) -> &str {
        match self {
            PublicUserDataNamespace::UserSettings => "userSettings",
        }
    }
}

impl From<PublicUserDataNamespace> for UserDataNamespace {
    fn from(value: PublicUserDataNamespace) -> Self {
        UserDataNamespace::Public(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::users::PublicUserDataNamespace;
    use insta::assert_json_snapshot;

    #[test]
    fn proper_str_reference() -> anyhow::Result<()> {
        assert_eq!(
            PublicUserDataNamespace::UserSettings.as_ref(),
            "userSettings"
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(PublicUserDataNamespace::UserSettings, @r###""userSettings""###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PublicUserDataNamespace>(r#""userSettings""#)?,
            PublicUserDataNamespace::UserSettings
        );

        Ok(())
    }
}
