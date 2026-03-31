use crate::utils::web_security::ContentSecurityPolicyContent;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "my-csp", "content": {"type": "serialized", "value": "default-src 'self'"}, "tagIds": []}))]
pub struct ContentSecurityPoliciesCreateParams {
    pub name: String,
    pub content: ContentSecurityPolicyContent,
    /// Tag IDs to assign to this content security policy.
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::{
        ContentSecurityPolicyContent, ContentSecurityPolicySource,
        api_ext::ContentSecurityPoliciesCreateParams,
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
                },
                tag_ids: vec![],
            }
        );

        Ok(())
    }
}
