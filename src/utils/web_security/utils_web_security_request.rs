use crate::utils::ContentSecurityPolicySource;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebSecurityRequest {
    #[serde(rename_all = "camelCase")]
    SerializeContentSecurityPolicy {
        policy_name: String,
        source: ContentSecurityPolicySource,
    },
}

#[cfg(test)]
mod tests {
    use crate::utils::{ContentSecurityPolicySource, UtilsWebSecurityRequest};

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityRequest>(
                r###"
{
    "type": "serializeContentSecurityPolicy",
    "value": { "policyName": "policy", "source": "meta" }
}
          "###
            )?,
            UtilsWebSecurityRequest::SerializeContentSecurityPolicy {
                policy_name: "policy".to_string(),
                source: ContentSecurityPolicySource::Meta,
            }
        );

        Ok(())
    }
}
