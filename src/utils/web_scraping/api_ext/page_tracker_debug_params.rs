use crate::{users::SecretsAccess, utils::web_scraping::page_trackers::PageTrackerTarget};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageTrackerDebugParams {
    pub target: PageTrackerTarget,
    pub secrets: SecretsAccess,
}

#[cfg(test)]
mod tests {
    use super::PageTrackerDebugParams;
    use crate::users::SecretsAccess;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let params: PageTrackerDebugParams = serde_json::from_str(
            r#"{ "target": { "extractor": "export async function execute(p) { return await p.content(); }" }, "secrets": { "type": "none" } }"#,
        )?;
        assert_eq!(
            params.target.extractor,
            "export async function execute(p) { return await p.content(); }"
        );
        assert!(!params.target.accept_invalid_certificates);
        assert_eq!(params.secrets, SecretsAccess::None);

        let params: PageTrackerDebugParams = serde_json::from_str(
            r#"{ "target": { "extractor": "export async function execute(p) { return await p.content(); }", "acceptInvalidCertificates": true }, "secrets": { "type": "all" } }"#,
        )?;
        assert!(params.target.accept_invalid_certificates);
        assert_eq!(params.secrets, SecretsAccess::All);

        let params: PageTrackerDebugParams = serde_json::from_str(
            r#"{ "target": { "extractor": "export async function execute(p) { return await p.content(); }" }, "secrets": { "type": "selected", "secrets": ["key1", "key2"] } }"#,
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
