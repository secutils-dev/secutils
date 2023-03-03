use crate::users::UserDataType;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PublicUserDataType {
    AutoResponders,
    ContentSecurityPolicies,
    SelfSignedCertificates,
    UserSettings,
}

impl From<PublicUserDataType> for UserDataType {
    fn from(value: PublicUserDataType) -> Self {
        UserDataType::Public(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::users::PublicUserDataType;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(PublicUserDataType::AutoResponders, @r###""autoResponders""###);
            assert_json_snapshot!(PublicUserDataType::ContentSecurityPolicies, @r###""contentSecurityPolicies""###);
            assert_json_snapshot!(PublicUserDataType::SelfSignedCertificates, @r###""selfSignedCertificates""###);
            assert_json_snapshot!(PublicUserDataType::UserSettings, @r###""userSettings""###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PublicUserDataType>(r###""autoResponders""###)?,
            PublicUserDataType::AutoResponders
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataType>(r###""contentSecurityPolicies""###)?,
            PublicUserDataType::ContentSecurityPolicies
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataType>(r###""selfSignedCertificates""###)?,
            PublicUserDataType::SelfSignedCertificates
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataType>(r###""userSettings""###)?,
            PublicUserDataType::UserSettings
        );

        Ok(())
    }
}
