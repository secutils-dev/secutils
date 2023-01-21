use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UserDataType {
    AutoResponders,
    ContentSecurityPolicies,
    SelfSignedCertificates,
    UserSettings,
}

impl UserDataType {
    pub fn get_data_key(&self) -> &str {
        match self {
            UserDataType::AutoResponders => "autoResponders",
            UserDataType::ContentSecurityPolicies => "contentSecurityPolicies",
            UserDataType::SelfSignedCertificates => "selfSignedCertificates",
            UserDataType::UserSettings => "userSettings",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserDataType;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(UserDataType::AutoResponders, @r###""autoResponders""###);
            assert_json_snapshot!(UserDataType::ContentSecurityPolicies, @r###""contentSecurityPolicies""###);
            assert_json_snapshot!(UserDataType::SelfSignedCertificates, @r###""selfSignedCertificates""###);
            assert_json_snapshot!(UserDataType::UserSettings, @r###""userSettings""###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""autoResponders""###)?,
            UserDataType::AutoResponders
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""contentSecurityPolicies""###)?,
            UserDataType::ContentSecurityPolicies
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""selfSignedCertificates""###)?,
            UserDataType::SelfSignedCertificates
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""userSettings""###)?,
            UserDataType::UserSettings
        );

        Ok(())
    }
}
