use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

/// Describe the responder path type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResponderPathType {
    /// Responder path is matched only if it is exactly the same as the request path.
    #[serde(rename = "=")]
    Exact,
    /// Responder path is matched if it is a prefix of the request path.
    #[serde(rename = "^")]
    Prefix,
}

impl FromStr for ResponderPathType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "=" => Ok(ResponderPathType::Exact),
            "^" => Ok(ResponderPathType::Prefix),
            path_type => Err(anyhow::anyhow!("Unsupported path type: {path_type}")),
        }
    }
}

impl Display for ResponderPathType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ResponderPathType::Exact => "=",
                ResponderPathType::Prefix => "^",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::ResponderPathType;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderPathType::Exact, @r###""=""###);
        assert_json_snapshot!(ResponderPathType::Prefix, @r###""^""###);

        assert_eq!(ResponderPathType::Exact.to_string(), "=");
        assert_eq!(ResponderPathType::Prefix.to_string(), "^");

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderPathType>(r#""=""#)?,
            ResponderPathType::Exact
        );
        assert_eq!(
            serde_json::from_str::<ResponderPathType>(r#""^""#)?,
            ResponderPathType::Prefix
        );

        assert_eq!("=".parse::<ResponderPathType>()?, ResponderPathType::Exact);
        assert_eq!("^".parse::<ResponderPathType>()?, ResponderPathType::Prefix);

        Ok(())
    }
}
