use crate::utils::ContentSecurityPolicyContent;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentSecurityPoliciesCreateParams {
    pub name: String,
    pub content: ContentSecurityPolicyContent,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ContentSecurityPoliciesCreateParams, ContentSecurityPolicyContent,
        ContentSecurityPolicySource,
    };
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPoliciesCreateParams>(
                r#"
{
    "name": "csp",
    "content": {
        "type": "remote",
        "value": { "url": "http://localhost:1234/my-page?one=two", "followRedirects": true, "source": "meta" }
    }
}
          "#
            )?,
            ContentSecurityPoliciesCreateParams {
                name: "csp".to_string(),
                content: ContentSecurityPolicyContent::Remote {
                    url: Url::parse("http://localhost:1234/my-page?one=two")?,
                    source: ContentSecurityPolicySource::Meta,
                    follow_redirects: true,
                }
            }
        );

        Ok(())
    }
}
