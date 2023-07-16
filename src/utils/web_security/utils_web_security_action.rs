use crate::{
    api::Api,
    users::{PublicUserDataNamespace, User},
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, ContentSecurityPolicy,
        ContentSecurityPolicyDirective, ContentSecurityPolicySource, UtilsWebSecurityActionResult,
    },
};
use anyhow::anyhow;
use serde::Deserialize;
use std::collections::BTreeMap;

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

    pub async fn handle(
        self,
        user: User,
        api: &Api,
    ) -> anyhow::Result<UtilsWebSecurityActionResult> {
        match self {
            UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name,
                source,
            } => {
                let policy = api
                    .users()
                    .get_data::<BTreeMap<String, ContentSecurityPolicy>>(
                        user.id,
                        PublicUserDataNamespace::ContentSecurityPolicies,
                    )
                    .await?
                    .and_then(|mut map| map.value.remove(&policy_name))
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find content security policy with name: {}",
                            policy_name
                        )
                    })?;

                let policy = match source {
                    ContentSecurityPolicySource::Meta => serialize_directives(
                        policy
                            .directives
                            .into_iter()
                            .filter(|directive| directive.is_supported_for_source(source)),
                    )?,
                    ContentSecurityPolicySource::Header => {
                        serialize_directives(policy.directives.into_iter())?
                    }
                };

                Ok(UtilsWebSecurityActionResult::SerializeContentSecurityPolicy { policy, source })
            }
        }
    }
}

fn serialize_directives(
    directives: impl Iterator<Item = ContentSecurityPolicyDirective>,
) -> anyhow::Result<String> {
    let mut serialized_directives = vec![];
    for directive in directives {
        serialized_directives.push(String::try_from(directive)?);
    }

    Ok(serialized_directives.join("; "))
}

#[cfg(test)]
mod tests {
    use super::serialize_directives;
    use crate::utils::{
        ContentSecurityPolicyDirective, ContentSecurityPolicySource, UtilsWebSecurityAction,
    };
    use insta::assert_debug_snapshot;
    use std::collections::HashSet;

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

    #[test]
    fn can_serialize_directives() -> anyhow::Result<()> {
        let directives = [
            ContentSecurityPolicyDirective::DefaultSrc(
                ["'self'".to_string(), "https:".to_string()]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::Sandbox(HashSet::new()),
            ContentSecurityPolicyDirective::ReportTo(["prod-csp".to_string()]),
        ];
        assert_debug_snapshot!(serialize_directives(directives.into_iter())?, @r###""default-src 'self' https:; sandbox; report-to prod-csp""###);

        Ok(())
    }
}
