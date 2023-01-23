use crate::utils::ContentSecurityPolicySource;
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebSecurityResponse {
    #[serde(rename_all = "camelCase")]
    GenerateContentSecurityPolicySnippet {
        snippet: String,
        source: ContentSecurityPolicySource,
    },
}

#[cfg(test)]
mod tests {
    use crate::utils::{ContentSecurityPolicySource, UtilsWebSecurityResponse};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsWebSecurityResponse::GenerateContentSecurityPolicySnippet {
            snippet: 
r###"Report-To: { "group": "csp-prod-group",
              "max_age": 10886400,
              "endpoints": [
                { "url": "https://prod.example.com/reports" },
                { "url": "https://staging.example.com/reports" }
              ]
           }
Content-Security-Policy: default-src: 'self'; script-src: https:; report-to csp-prod-group"###.to_string(),
            source: ContentSecurityPolicySource::Header
        }, @r###"
        {
          "type": "generateContentSecurityPolicySnippet",
          "value": {
            "snippet": "Report-To: { \"group\": \"csp-prod-group\",\n              \"max_age\": 10886400,\n              \"endpoints\": [\n                { \"url\": \"https://prod.example.com/reports\" },\n                { \"url\": \"https://staging.example.com/reports\" }\n              ]\n           }\nContent-Security-Policy: default-src: 'self'; script-src: https:; report-to csp-prod-group",
            "source": "header"
          }
        }
        "###);

        Ok(())
    }
}
