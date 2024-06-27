use crate::utils::webhooks::ResponderPathType;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display, Formatter},
    str::FromStr,
};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderLocation {
    /// Responder location path type.
    pub path_type: ResponderPathType,
    /// Responder location path.
    pub path: String,
    /// Optional subdomain to match. If not specified, root domain is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdomain: Option<String>,
}

impl Display for ResponderLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.subdomain
                .as_deref()
                .unwrap_or("@")
                .to_ascii_lowercase(),
            self.path_type,
            self.path.to_ascii_lowercase()
        )
    }
}

impl Debug for ResponderLocation {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self.subdomain {
            Some(ref subdomain) => write!(
                f,
                "{} ({}, {:?})",
                self.path.to_ascii_lowercase(),
                subdomain.to_ascii_lowercase(),
                self.path_type
            ),
            None => write!(
                f,
                "{} ({:?})",
                self.path.to_ascii_lowercase(),
                self.path_type
            ),
        }
    }
}

impl FromStr for ResponderLocation {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(3, ':').collect();
        if parts.len() == 3 {
            if let Ok(path_type) = parts[1].parse() {
                return Ok(ResponderLocation {
                    subdomain: match parts[0] {
                        "@" => None,
                        subdomain => Some(subdomain.to_ascii_lowercase()),
                    },
                    path_type,
                    path: parts[2].to_ascii_lowercase(),
                });
            }
        }
        Err(anyhow::anyhow!("Invalid location format: {s}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::{ResponderLocation, ResponderPathType};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let location = ResponderLocation {
            path_type: ResponderPathType::Exact,
            path: "/pAth".to_string(),
            subdomain: None,
        };
        assert_json_snapshot!(location, @r###"
        {
          "pathType": "=",
          "path": "/pAth"
        }
        "###);
        assert_eq!(location.to_string(), "@:=:/path");

        let location = ResponderLocation {
            path_type: ResponderPathType::Prefix,
            path: "/paTh".to_string(),
            subdomain: Some("mY.domAiN".to_string()),
        };
        assert_json_snapshot!(location, @r###"
        {
          "pathType": "^",
          "path": "/paTh",
          "subdomain": "mY.domAiN"
        }
        "###);
        assert_eq!(location.to_string(), "my.domain:^:/path");

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderLocation>(
                r#"
        {
          "pathType": "=",
          "path": "/pAth"
        }
        "#
            )?,
            ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/pAth".to_string(),
                subdomain: None,
            }
        );
        assert_eq!(
            "@:=:/pAth".parse::<ResponderLocation>()?,
            ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/path".to_string(),
                subdomain: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ResponderLocation>(
                r#"
        {
          "pathType": "^",
          "path": "/paTh",
          "subdomain": "mY.domain"
        }
        "#
            )?,
            ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/paTh".to_string(),
                subdomain: Some("mY.domain".to_string()),
            }
        );
        assert_eq!(
            "mY.domAin:^:/paTh".parse::<ResponderLocation>()?,
            ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/path".to_string(),
                subdomain: Some("my.domain".to_string()),
            }
        );

        Ok(())
    }
}
