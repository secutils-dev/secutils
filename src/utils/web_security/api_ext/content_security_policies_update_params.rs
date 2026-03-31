use crate::utils::web_security::ContentSecurityPolicyDirective;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "renamed-csp"}))]
pub struct ContentSecurityPoliciesUpdateParams {
    pub name: Option<String>,
    pub directives: Option<Vec<ContentSecurityPolicyDirective>>,
    /// Tag IDs to assign. When `Some`, replaces all tags; when `None`, tags are unchanged.
    pub tag_ids: Option<Vec<Uuid>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::{
        ContentSecurityPolicyDirective, api_ext::ContentSecurityPoliciesUpdateParams,
    };

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPoliciesUpdateParams>(
                r#"
        {
            "name": "csp",
            "directives": [{"name": "child-src", "value": ["'self'", "https://*"]}]
        }
                  "#
            )?,
            ContentSecurityPoliciesUpdateParams {
                name: Some("csp".to_string()),
                directives: Some(vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string(), "https://*".to_string()]
                        .into_iter()
                        .collect()
                )]),
                tag_ids: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPoliciesUpdateParams>(
                r#"
        {
            "directives": [{"name": "child-src", "value": ["'self'", "https://*"]}]
        }
                  "#
            )?,
            ContentSecurityPoliciesUpdateParams {
                name: None,
                directives: Some(vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string(), "https://*".to_string()]
                        .into_iter()
                        .collect()
                )]),
                tag_ids: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPoliciesUpdateParams>(
                r#"
        {
            "name": "csp"
        }
                  "#
            )?,
            ContentSecurityPoliciesUpdateParams {
                name: Some("csp".to_string()),
                directives: None,
                tag_ids: None,
            }
        );

        Ok(())
    }
}
