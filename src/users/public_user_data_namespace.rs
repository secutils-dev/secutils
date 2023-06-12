use crate::users::UserDataNamespace;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PublicUserDataNamespace {
    AutoResponders,
    ContentSecurityPolicies,
    SelfSignedCertificates,
    UserSettings,
    WebPageResourcesTrackers,
}

impl AsRef<str> for PublicUserDataNamespace {
    fn as_ref(&self) -> &str {
        match self {
            PublicUserDataNamespace::AutoResponders => "autoResponders",
            PublicUserDataNamespace::ContentSecurityPolicies => "contentSecurityPolicies",
            PublicUserDataNamespace::SelfSignedCertificates => "selfSignedCertificates",
            PublicUserDataNamespace::UserSettings => "userSettings",
            PublicUserDataNamespace::WebPageResourcesTrackers => "webPageResourcesTrackers",
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
            PublicUserDataNamespace::AutoResponders.as_ref(),
            "autoResponders"
        );

        assert_eq!(
            PublicUserDataNamespace::ContentSecurityPolicies.as_ref(),
            "contentSecurityPolicies"
        );

        assert_eq!(
            PublicUserDataNamespace::SelfSignedCertificates.as_ref(),
            "selfSignedCertificates"
        );

        assert_eq!(
            PublicUserDataNamespace::UserSettings.as_ref(),
            "userSettings"
        );

        assert_eq!(
            PublicUserDataNamespace::WebPageResourcesTrackers.as_ref(),
            "webPageResourcesTrackers"
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(PublicUserDataNamespace::AutoResponders, @r###""autoResponders""###);
            assert_json_snapshot!(PublicUserDataNamespace::ContentSecurityPolicies, @r###""contentSecurityPolicies""###);
            assert_json_snapshot!(PublicUserDataNamespace::SelfSignedCertificates, @r###""selfSignedCertificates""###);
            assert_json_snapshot!(PublicUserDataNamespace::UserSettings, @r###""userSettings""###);
            assert_json_snapshot!(PublicUserDataNamespace::WebPageResourcesTrackers, @r###""webPageResourcesTrackers""###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PublicUserDataNamespace>(r###""autoResponders""###)?,
            PublicUserDataNamespace::AutoResponders
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataNamespace>(r###""contentSecurityPolicies""###)?,
            PublicUserDataNamespace::ContentSecurityPolicies
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataNamespace>(r###""selfSignedCertificates""###)?,
            PublicUserDataNamespace::SelfSignedCertificates
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataNamespace>(r###""userSettings""###)?,
            PublicUserDataNamespace::UserSettings
        );

        assert_eq!(
            serde_json::from_str::<PublicUserDataNamespace>(r###""webPageResourcesTrackers""###)?,
            PublicUserDataNamespace::WebPageResourcesTrackers
        );

        Ok(())
    }
}
