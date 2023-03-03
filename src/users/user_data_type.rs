use crate::users::{InternalUserDataType, PublicUserDataType};
use serde_derive::Serialize;

#[derive(Serialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(untagged, rename_all = "camelCase")]
pub enum UserDataType {
    Public(PublicUserDataType),
    Internal(InternalUserDataType),
}

#[cfg(test)]
mod tests {
    use crate::users::{InternalUserDataType, PublicUserDataType, UserDataType};
    use insta::assert_json_snapshot;

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
}
