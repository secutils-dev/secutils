use crate::utils::{ContentSecurityPolicyDirective, ContentSecurityPolicySource};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum ContentSecurityPolicyContent {
    Directives(Vec<ContentSecurityPolicyDirective>),
    Serialized(String),
    #[serde(rename_all = "camelCase")]
    Remote {
        url: Url,
        source: ContentSecurityPolicySource,
        follow_redirects: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::ContentSecurityPolicyContent;
    use crate::utils::{ContentSecurityPolicyDirective, ContentSecurityPolicySource};
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyContent>(
                r#"{"type": "serialized", "value": "default-src 'self' https:"}"#
            )?,
            ContentSecurityPolicyContent::Serialized("default-src 'self' https:".to_string())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyContent>(
                r#"{
                  "type": "remote",
                  "value": { "url": "http://localhost:1234/my-page?one=two", "followRedirects": true, "source": "meta" }
                  }"#
            )?,
            ContentSecurityPolicyContent::Remote {
                url: Url::parse("http://localhost:1234/my-page?one=two")?,
                source: ContentSecurityPolicySource::Meta,
                follow_redirects: true,
            }
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyContent>(
                r#"{
                  "type": "directives",
                  "value": [{"name": "child-src", "value": ["'self'", "https://*"]}]
                  }"#
            )?,
            ContentSecurityPolicyContent::Directives(vec![
                ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string(), "https://*".to_string()]
                        .into_iter()
                        .collect()
                )
            ])
        );

        Ok(())
    }
}
