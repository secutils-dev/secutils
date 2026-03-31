use crate::utils::web_security::ContentSecurityPolicySource;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"source": "enforcingHeader"}))]
pub struct ContentSecurityPoliciesSerializeParams {
    pub source: ContentSecurityPolicySource,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::{
        ContentSecurityPolicySource, api_ext::ContentSecurityPoliciesSerializeParams,
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
