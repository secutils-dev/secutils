use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

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

impl Display for PrivateKeySize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u32)
    }
}

impl FromStr for PrivateKeySize {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1024" => Ok(PrivateKeySize::Size1024),
            "2048" => Ok(PrivateKeySize::Size2048),
            "4096" => Ok(PrivateKeySize::Size4096),
            "8192" => Ok(PrivateKeySize::Size8192),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::PrivateKeySize;
    use insta::{assert_json_snapshot, assert_snapshot};
    use std::str::FromStr;

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

    #[test]
    fn string_representation() -> anyhow::Result<()> {
        assert_snapshot!(PrivateKeySize::Size1024, @"1024");
        assert_snapshot!(PrivateKeySize::Size2048, @"2048");
        assert_snapshot!(PrivateKeySize::Size4096, @"4096");
        assert_snapshot!(PrivateKeySize::Size8192, @"8192");

        assert_eq!(
            PrivateKeySize::from_str("1024"),
            Ok(PrivateKeySize::Size1024)
        );
        assert_eq!(
            PrivateKeySize::from_str("2048"),
            Ok(PrivateKeySize::Size2048)
        );
        assert_eq!(
            PrivateKeySize::from_str("4096"),
            Ok(PrivateKeySize::Size4096)
        );
        assert_eq!(
            PrivateKeySize::from_str("8192"),
            Ok(PrivateKeySize::Size8192)
        );

        Ok(())
    }
}
