use crate::utils::{
    utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, ContentSecurityPolicySource,
};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebSecurityAction {
    #[serde(rename_all = "camelCase")]
    SerializeContentSecurityPolicy {
        policy_name: String,
        source: ContentSecurityPolicySource,
    },
}

impl UtilsWebSecurityAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsWebSecurityAction::SerializeContentSecurityPolicy { policy_name, .. } => {
                if policy_name.is_empty() {
                    anyhow::bail!("Policy name cannot be empty");
                }

                if policy_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Policy name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{ContentSecurityPolicySource, UtilsWebSecurityAction};
    use insta::assert_debug_snapshot;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r###"
{
    "type": "serializeContentSecurityPolicy",
    "value": { "policyName": "policy", "source": "meta" }
}
          "###
            )?,
            UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "policy".to_string(),
                source: ContentSecurityPolicySource::Meta,
            }
        );

        Ok(())
    }

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: "a".repeat(100),
            source: ContentSecurityPolicySource::Meta,
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: "".to_string(),
            source: ContentSecurityPolicySource::Meta,
        }
        .validate(), @r###"
        Err(
            "Policy name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: "a".repeat(101),
            source: ContentSecurityPolicySource::Meta,
        }
        .validate(), @r###"
        Err(
            "Policy name cannot be longer than 100 characters",
        )
        "###);

        Ok(())
    }
}
