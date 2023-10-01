use crate::utils::{
    utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, ContentSecurityPolicyDirective,
};
use serde::{Deserialize, Serialize};

/// Represents content security policy (CSP) with the arbitrary name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentSecurityPolicy {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "d")]
    pub directives: Vec<ContentSecurityPolicyDirective>,
}

impl ContentSecurityPolicy {
    /// Performs basic content security policy validation.
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= MAX_UTILS_ENTITY_NAME_LENGTH
            && !self.directives.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{ContentSecurityPolicy, ContentSecurityPolicyDirective};
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ContentSecurityPolicy {
            name: "some-name".to_string(),
            directives: vec![
                ContentSecurityPolicyDirective::ChildSrc(["'self'".to_string()].into_iter().collect())
            ]
        }, @r###"
        {
          "n": "some-name",
          "d": [
            {
              "n": "child-src",
              "v": [
                "'self'"
              ]
            }
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicy>(
                &json!({ "n": "some-name", "d": [{"n": "child-src", "v": ["'self'", "https://*"]}] })
                    .to_string()
            )?,
            ContentSecurityPolicy {
                name: "some-name".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string(), "https://*".to_string()]
                        .into_iter()
                        .collect()
                )]
            }
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicy>(
                &json!({ "n": "some-name", "d": [{"n": "sandbox", "v": []}] }).to_string()
            )?,
            ContentSecurityPolicy {
                name: "some-name".to_string(),
                directives: vec![ContentSecurityPolicyDirective::Sandbox(HashSet::new())]
            }
        );

        Ok(())
    }

    #[test]
    fn is_valid() {
        assert!(!ContentSecurityPolicy {
            name: "".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect()
            )]
        }
        .is_valid());
        assert!(!ContentSecurityPolicy {
            name: "some-name".to_string(),
            directives: vec![]
        }
        .is_valid());
        assert!(ContentSecurityPolicy {
            name: "some-name".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect()
            )]
        }
        .is_valid());
    }
}
