use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UserDataType {
    AutoResponders,
    SelfSignedCertificates,
}

impl UserDataType {
    pub fn get_data_key(&self) -> &str {
        match self {
            UserDataType::AutoResponders => "autoResponders",
            UserDataType::SelfSignedCertificates => "selfSignedCertificates",
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
            assert_json_snapshot!(UserDataType::SelfSignedCertificates, @r###""selfSignedCertificates""###);
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
            serde_json::from_str::<UserDataType>(r###""selfSignedCertificates""###)?,
            UserDataType::SelfSignedCertificates
        );

        Ok(())
    }
}
