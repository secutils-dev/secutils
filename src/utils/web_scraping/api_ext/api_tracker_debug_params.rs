use crate::{users::SecretsAccess, utils::web_scraping::api_trackers::ApiTrackerTarget};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiTrackerDebugParams {
    pub target: ApiTrackerTarget,
    pub secrets: SecretsAccess,
}

#[cfg(test)]
mod tests {
    use super::ApiTrackerDebugParams;
    use crate::users::SecretsAccess;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let params: ApiTrackerDebugParams = serde_json::from_str(
            r#"{ "target": { "url": "https://api.example.com/data" }, "secrets": { "type": "none" } }"#,
        )?;
        assert_eq!(params.target.url.as_str(), "https://api.example.com/data");
        assert!(params.target.method.is_none());
        assert!(params.target.configurator.is_none());
        assert!(params.target.extractor.is_none());
        assert_eq!(params.secrets, SecretsAccess::None);

        let params: ApiTrackerDebugParams = serde_json::from_str(
            r#"{ "target": { "url": "https://api.example.com/data", "method": "POST", "configurator": "(() => context)()", "extractor": "(() => context.body)()" }, "secrets": { "type": "all" } }"#,
        )?;
        assert_eq!(params.target.method.as_deref(), Some("POST"));
        assert_eq!(
            params.target.configurator.as_deref(),
            Some("(() => context)()")
        );
        assert_eq!(
            params.target.extractor.as_deref(),
            Some("(() => context.body)()")
        );
        assert_eq!(params.secrets, SecretsAccess::All);

        let params: ApiTrackerDebugParams = serde_json::from_str(
            r#"{ "target": { "url": "https://api.example.com/data" }, "secrets": { "type": "selected", "secrets": ["key1", "key2"] } }"#,
        )?;
        assert_eq!(
            params.secrets,
            SecretsAccess::Selected {
                secrets: vec!["key1".to_string(), "key2".to_string()]
            }
        );

        Ok(())
    }
}
