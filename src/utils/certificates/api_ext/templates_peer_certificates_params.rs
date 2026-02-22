use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesPeerCertificatesParams {
    pub url: Url,
}

#[cfg(test)]
mod tests {
    use super::TemplatesPeerCertificatesParams;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<TemplatesPeerCertificatesParams>(
                r#"{ "url": "https://example.com" }"#
            )?,
            TemplatesPeerCertificatesParams {
                url: Url::parse("https://example.com")?,
            }
        );

        Ok(())
    }
}
