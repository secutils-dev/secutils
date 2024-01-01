use crate::utils::web_security::ContentSecurityPolicyDirective;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentSecurityPoliciesUpdateParams {
    pub name: Option<String>,
    pub directives: Option<Vec<ContentSecurityPolicyDirective>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::{
        api_ext::ContentSecurityPoliciesUpdateParams, ContentSecurityPolicyDirective,
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
                )])
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
                )])
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
                directives: None
            }
        );

        Ok(())
    }
}
