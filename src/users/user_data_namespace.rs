use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UserDataNamespace {
    UserSettings,
}

impl AsRef<str> for UserDataNamespace {
    fn as_ref(&self) -> &str {
        match self {
            UserDataNamespace::UserSettings => "userSettings",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserDataNamespace;
    use insta::assert_json_snapshot;

    #[test]
    fn proper_str_reference() -> anyhow::Result<()> {
        assert_eq!(UserDataNamespace::UserSettings.as_ref(), "userSettings");

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(UserDataNamespace::UserSettings, @r###""userSettings""###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UserDataNamespace>(r#""userSettings""#)?,
            UserDataNamespace::UserSettings
        );

        Ok(())
    }
}
