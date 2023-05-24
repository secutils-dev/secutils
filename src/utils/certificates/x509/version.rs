use serde_repr::*;

/// Represents X.509 certificate version.
#[derive(Serialize_repr, Deserialize_repr, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Version {
    One = 1,
    Two = 2,
    Three = 3,
}

impl Version {
    /// Value of the X.509 certificate version according to https://www.ietf.org/rfc/rfc5280.html#section-4.1.2.1.
    pub const fn value(&self) -> i32 {
        match self {
            Version::One => 0,
            Version::Two => 1,
            Version::Three => 2,
        }
    }

    /// Latest known version of the X.509 certificate.
    pub const fn latest() -> Self {
        Self::Three
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::Version;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(Version::One, @r###"1"###);
        assert_json_snapshot!(Version::Two, @r###"2"###);
        assert_json_snapshot!(Version::Three, @r###"3"###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(serde_json::from_str::<Version>(r###"1"###)?, Version::One);
        assert_eq!(serde_json::from_str::<Version>(r###"2"###)?, Version::Two);
        assert_eq!(serde_json::from_str::<Version>(r###"3"###)?, Version::Three);

        assert!(serde_json::from_str::<Version>(r###"-1"###).is_err());
        assert!(serde_json::from_str::<Version>(r###"0"###).is_err());
        assert!(serde_json::from_str::<Version>(r###"4"###).is_err());

        Ok(())
    }

    #[test]
    fn correctly_returns_latest() {
        assert_eq!(Version::latest(), Version::Three);
    }

    #[test]
    fn correctly_returns_value() {
        assert_eq!(Version::One.value(), 0);
        assert_eq!(Version::Two.value(), 1);
        assert_eq!(Version::Three.value(), 2);
    }
}
