use crate::users::UserDataType;
use serde_derive::Serialize;

#[derive(Serialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum InternalUserDataType {
    AccountActivationToken,
    PasswordResetToken,
}

impl From<InternalUserDataType> for UserDataType {
    fn from(value: InternalUserDataType) -> Self {
        UserDataType::Internal(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::users::InternalUserDataType;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(InternalUserDataType::AccountActivationToken, @r###""accountActivationToken""###);
            assert_json_snapshot!(InternalUserDataType::PasswordResetToken, @r###""passwordResetToken""###);
        });

        Ok(())
    }
}
