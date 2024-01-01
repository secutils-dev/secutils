use crate::utils::web_security::ContentSecurityPolicySource;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentSecurityPoliciesSerializeParams {
    pub source: ContentSecurityPolicySource,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::{
        api_ext::ContentSecurityPoliciesSerializeParams, ContentSecurityPolicySource,
    };

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPoliciesSerializeParams>(
                r#"
{
    "source": "enforcingHeader"
}
          "#
            )?,
            ContentSecurityPoliciesSerializeParams {
                source: ContentSecurityPolicySource::EnforcingHeader
            }
        );

        Ok(())
    }
}
