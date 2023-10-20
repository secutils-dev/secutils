use serde::{Deserialize, Serialize};

/// The key size defines a number of bits in a key used by a cryptographic algorithm.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PrivateKeySize {
    #[serde(rename = "1024")]
    Size1024 = 1024,
    #[serde(rename = "2048")]
    Size2048 = 2048,
    #[serde(rename = "4096")]
    Size4096 = 4096,
    #[serde(rename = "8192")]
    Size8192 = 8192,
}

#[cfg(test)]
mod tests {
    use crate::utils::PrivateKeySize;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() {
        assert_json_snapshot!(PrivateKeySize::Size1024, @r###""1024""###);
        assert_json_snapshot!(PrivateKeySize::Size2048, @r###""2048""###);
        assert_json_snapshot!(PrivateKeySize::Size4096, @r###""4096""###);
        assert_json_snapshot!(PrivateKeySize::Size8192, @r###""8192""###);
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PrivateKeySize>(r#""1024""#)?,
            PrivateKeySize::Size1024
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeySize>(r#""2048""#)?,
            PrivateKeySize::Size2048
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeySize>(r#""4096""#)?,
            PrivateKeySize::Size4096
        );

        assert_eq!(
            serde_json::from_str::<PrivateKeySize>(r#""8192""#)?,
            PrivateKeySize::Size8192
        );

        Ok(())
    }

    #[test]
    fn as_number() {
        assert_eq!(PrivateKeySize::Size1024 as u32, 1024);
        assert_eq!(PrivateKeySize::Size2048 as u32, 2048);
        assert_eq!(PrivateKeySize::Size4096 as u32, 4096);
        assert_eq!(PrivateKeySize::Size8192 as u32, 8192);
    }
}