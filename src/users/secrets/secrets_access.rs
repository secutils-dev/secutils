use serde::{Deserialize, Serialize};

/// Controls which user secrets are exposed to a responder or tracker script.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SecretsAccess {
    /// No secrets are exposed (default).
    #[default]
    None,
    /// All user secrets are exposed.
    All,
    /// Only the named secrets are exposed.
    Selected { secrets: Vec<String> },
}

/// Postcard-compatible mirror of [`SecretsAccess`].
///
/// The primary enum uses `#[serde(tag = "type")]` (internally tagged) for clean JSON,
/// but postcard cannot round-trip that representation because it requires `deserialize_any`.
/// This enum uses serde's default (index-based) encoding which postcard handles natively.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawSecretsAccess {
    None,
    All,
    Selected(Vec<String>),
}

impl From<&SecretsAccess> for RawSecretsAccess {
    fn from(sa: &SecretsAccess) -> Self {
        match sa {
            SecretsAccess::None => RawSecretsAccess::None,
            SecretsAccess::All => RawSecretsAccess::All,
            SecretsAccess::Selected { secrets } => RawSecretsAccess::Selected(secrets.clone()),
        }
    }
}

impl From<RawSecretsAccess> for SecretsAccess {
    fn from(raw: RawSecretsAccess) -> Self {
        match raw {
            RawSecretsAccess::None => SecretsAccess::None,
            RawSecretsAccess::All => SecretsAccess::All,
            RawSecretsAccess::Selected(secrets) => SecretsAccess::Selected { secrets },
        }
    }
}

impl SecretsAccess {
    /// Returns `true` if this is the `None` variant.
    pub fn is_none(&self) -> bool {
        matches!(self, SecretsAccess::None)
    }

    /// Removes a secret name from a `Selected` list. If the list becomes empty, returns `None`.
    pub fn without_secret(&self, name: &str) -> SecretsAccess {
        match self {
            SecretsAccess::Selected { secrets } => {
                let filtered: Vec<String> =
                    secrets.iter().filter(|s| s != &name).cloned().collect();
                if filtered.is_empty() {
                    SecretsAccess::None
                } else {
                    SecretsAccess::Selected { secrets: filtered }
                }
            }
            other => other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SecretsAccess;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() {
        assert_json_snapshot!(SecretsAccess::None, @r###"
        {
          "type": "none"
        }
        "###);
        assert_json_snapshot!(SecretsAccess::All, @r###"
        {
          "type": "all"
        }
        "###);
        assert_json_snapshot!(SecretsAccess::Selected { secrets: vec!["A".into(), "B".into()] }, @r#"
        {
          "type": "selected",
          "secrets": [
            "A",
            "B"
          ]
        }
        "#);
    }

    #[test]
    fn deserialization() {
        assert_eq!(
            serde_json::from_str::<SecretsAccess>(r#"{"type":"none"}"#).unwrap(),
            SecretsAccess::None
        );
        assert_eq!(
            serde_json::from_str::<SecretsAccess>(r#"{"type":"all"}"#).unwrap(),
            SecretsAccess::All
        );
        assert_eq!(
            serde_json::from_str::<SecretsAccess>(r#"{"type":"selected","secrets":["X"]}"#)
                .unwrap(),
            SecretsAccess::Selected {
                secrets: vec!["X".into()]
            }
        );
    }

    #[test]
    fn is_none() {
        assert!(SecretsAccess::None.is_none());
        assert!(!SecretsAccess::All.is_none());
        assert!(
            !SecretsAccess::Selected {
                secrets: vec!["A".into()]
            }
            .is_none()
        );
    }

    #[test]
    fn without_secret() {
        assert_eq!(SecretsAccess::None.without_secret("X"), SecretsAccess::None);
        assert_eq!(SecretsAccess::All.without_secret("X"), SecretsAccess::All);

        let selected = SecretsAccess::Selected {
            secrets: vec!["A".into(), "B".into(), "C".into()],
        };
        assert_eq!(
            selected.without_secret("B"),
            SecretsAccess::Selected {
                secrets: vec!["A".into(), "C".into()]
            }
        );

        let single = SecretsAccess::Selected {
            secrets: vec!["ONLY".into()],
        };
        assert_eq!(single.without_secret("ONLY"), SecretsAccess::None);
    }

    #[test]
    fn default_is_none() {
        assert_eq!(SecretsAccess::default(), SecretsAccess::None);
    }
}
