use crate::utils::ContentSecurityPolicySource;
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebSecurityActionResult {
    #[serde(rename_all = "camelCase")]
    SerializeContentSecurityPolicy {
        policy: String,
        source: ContentSecurityPolicySource,
    },
}

#[cfg(test)]
mod tests {
    use crate::utils::{ContentSecurityPolicySource, UtilsWebSecurityActionResult};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsWebSecurityActionResult::SerializeContentSecurityPolicy {
            policy: r###"default-src: 'self'; script-src: https:; report-to csp-prod-group"###.to_string(),
            source: ContentSecurityPolicySource::Header
        }, @r###"
        {
          "type": "serializeContentSecurityPolicy",
          "value": {
            "policy": "default-src: 'self'; script-src: https:; report-to csp-prod-group",
            "source": "header"
          }
        }
        "###);

        Ok(())
    }
}
