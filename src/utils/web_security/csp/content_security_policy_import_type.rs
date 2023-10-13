use crate::utils::ContentSecurityPolicySource;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum ContentSecurityPolicyImportType {
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Url {
        url: Url,
        source: ContentSecurityPolicySource,
        follow_redirects: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::ContentSecurityPolicyImportType;
    use crate::utils::ContentSecurityPolicySource;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyImportType>(
                r#"{"type": "text", "text": "default-src 'self' https:"}"#
            )?,
            ContentSecurityPolicyImportType::Text {
                text: "default-src 'self' https:".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyImportType>(
                r#"{
                  "type": "url",
                   "url": "http://localhost:1234/my-page?one=two",
                   "followRedirects": true,
                   "source": "meta"
                  }"#
            )?,
            ContentSecurityPolicyImportType::Url {
                url: Url::parse("http://localhost:1234/my-page?one=two")?,
                source: ContentSecurityPolicySource::Meta,
                follow_redirects: true,
            }
        );

        Ok(())
    }
}
