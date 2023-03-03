use crate::users::{InternalUserDataType, PublicUserDataType};
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(untagged, rename_all = "camelCase")]
pub enum UserDataType {
    Public(PublicUserDataType),
    Internal(InternalUserDataType),
}

impl UserDataType {
    pub fn get_data_key(&self) -> &str {
        match self {
            UserDataType::Public(data_type) => match data_type {
                PublicUserDataType::AutoResponders => "autoResponders",
                PublicUserDataType::ContentSecurityPolicies => "contentSecurityPolicies",
                PublicUserDataType::SelfSignedCertificates => "selfSignedCertificates",
                PublicUserDataType::UserSettings => "userSettings",
            },
            UserDataType::Internal(data_type) => match data_type {
                InternalUserDataType::AccountActivationToken => "accountActivationToken",
                InternalUserDataType::PasswordResetToken => "passwordResetToken",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::users::{InternalUserDataType, PublicUserDataType, UserDataType};
    use insta::assert_json_snapshot;

    #[test]
    fn gets_proper_data_key() -> anyhow::Result<()> {
        assert_eq!(
            UserDataType::Public(PublicUserDataType::AutoResponders).get_data_key(),
            "autoResponders"
        );
        assert_eq!(
            UserDataType::Public(PublicUserDataType::ContentSecurityPolicies).get_data_key(),
            "contentSecurityPolicies"
        );
        assert_eq!(
            UserDataType::Public(PublicUserDataType::SelfSignedCertificates).get_data_key(),
            "selfSignedCertificates"
        );
        assert_eq!(
            UserDataType::Public(PublicUserDataType::UserSettings).get_data_key(),
            "userSettings"
        );
        assert_eq!(
            UserDataType::Internal(InternalUserDataType::AccountActivationToken).get_data_key(),
            "accountActivationToken"
        );
        assert_eq!(
            UserDataType::Internal(InternalUserDataType::PasswordResetToken).get_data_key(),
            "passwordResetToken"
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(UserDataType::Public(PublicUserDataType::AutoResponders), @r###""autoResponders""###);
            assert_json_snapshot!(UserDataType::Public(PublicUserDataType::ContentSecurityPolicies), @r###""contentSecurityPolicies""###);
            assert_json_snapshot!(UserDataType::Public(PublicUserDataType::SelfSignedCertificates), @r###""selfSignedCertificates""###);
            assert_json_snapshot!(UserDataType::Public(PublicUserDataType::UserSettings), @r###""userSettings""###);
            assert_json_snapshot!(UserDataType::Internal(InternalUserDataType::AccountActivationToken), @r###""accountActivationToken""###);
            assert_json_snapshot!(UserDataType::Internal(InternalUserDataType::PasswordResetToken), @r###""passwordResetToken""###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""autoResponders""###)?,
            UserDataType::Public(PublicUserDataType::AutoResponders)
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""contentSecurityPolicies""###)?,
            UserDataType::Public(PublicUserDataType::ContentSecurityPolicies)
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""selfSignedCertificates""###)?,
            UserDataType::Public(PublicUserDataType::SelfSignedCertificates)
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""userSettings""###)?,
            UserDataType::Public(PublicUserDataType::UserSettings)
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""accountActivationToken""###)?,
            UserDataType::Internal(InternalUserDataType::AccountActivationToken)
        );

        assert_eq!(
            serde_json::from_str::<UserDataType>(r###""passwordResetToken""###)?,
            UserDataType::Internal(InternalUserDataType::PasswordResetToken)
        );

        Ok(())
    }
}
