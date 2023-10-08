use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StatusLevel {
    Available,
    Unavailable,
}

#[cfg(test)]
mod tests {
    use crate::server::StatusLevel;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::to_string(&StatusLevel::Available)?,
            r#""available""#
        );
        assert_eq!(
            serde_json::to_string(&StatusLevel::Unavailable)?,
            r#""unavailable""#
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<StatusLevel>(r#""available""#)?,
            StatusLevel::Available
        );
        assert_eq!(
            serde_json::from_str::<StatusLevel>(r#""unavailable""#)?,
            StatusLevel::Unavailable
        );

        Ok(())
    }
}
