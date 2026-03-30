use serde::Deserialize;
use url::Url;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"url": "https://example.com"}))]
pub struct TemplatesFetchCertificatesParams {
    #[schema(value_type = String)]
    pub url: Url,
}

#[cfg(test)]
mod tests {
    use super::TemplatesFetchCertificatesParams;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<TemplatesFetchCertificatesParams>(
                r#"{ "url": "https://example.com" }"#
            )?,
            TemplatesFetchCertificatesParams {
                url: Url::parse("https://example.com")?,
            }
        );

        Ok(())
    }
}
