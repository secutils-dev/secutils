use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UserDataType {
    AutoResponders,
    RootCertificates,
}

impl UserDataType {
    pub fn get_data_key(&self) -> &str {
        match self {
            UserDataType::AutoResponders => "autoResponders",
            UserDataType::RootCertificates => "rootCertificates",
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
            assert_json_snapshot!(UserDataType::RootCertificates, @r###""rootCertificates""###);
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
            serde_json::from_str::<UserDataType>(r###""rootCertificates""###)?,
            UserDataType::RootCertificates
        );

        Ok(())
    }
}
