use retrack_types::trackers::ExtractorEngine;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageTrackerTarget {
    /// A custom script (Playwright scenario) to extract data from the page.
    pub extractor: String,
    /// Whether to accept invalid TLS certificates.
    #[serde(default)]
    pub accept_invalid_certificates: bool,
    /// The browser engine to use for content extraction. Defaults to Chromium when absent.
    pub engine: Option<ExtractorEngine>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::page_trackers::PageTrackerTarget;
    use retrack_types::trackers::ExtractorEngine;
    use serde_json::json;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let target = PageTrackerTarget {
            extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
            accept_invalid_certificates: false,
            engine: None,
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerTarget>(
                &json!({ "extractor": "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }" }).to_string()
            )?,
            target
        );

        Ok(())
    }

    #[test]
    fn deserialization_with_accept_invalid_certificates() -> anyhow::Result<()> {
        let target = PageTrackerTarget {
            extractor: "export async function execute(p) { return await p.content(); }".to_string(),
            accept_invalid_certificates: true,
            engine: None,
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerTarget>(
                &json!({
                    "extractor": "export async function execute(p) { return await p.content(); }",
                    "acceptInvalidCertificates": true
                })
                .to_string()
            )?,
            target
        );

        Ok(())
    }

    #[test]
    fn deserialization_with_engine() -> anyhow::Result<()> {
        let target = PageTrackerTarget {
            extractor: "export async function execute(p) { return await p.content(); }".to_string(),
            accept_invalid_certificates: false,
            engine: Some(ExtractorEngine::Camoufox),
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerTarget>(
                &json!({
                    "extractor": "export async function execute(p) { return await p.content(); }",
                    "engine": { "type": "camoufox" }
                })
                .to_string()
            )?,
            target
        );

        let target_chromium = PageTrackerTarget {
            extractor: "export async function execute(p) { return await p.content(); }".to_string(),
            accept_invalid_certificates: false,
            engine: Some(ExtractorEngine::Chromium),
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerTarget>(
                &json!({
                    "extractor": "export async function execute(p) { return await p.content(); }",
                    "engine": { "type": "chromium" }
                })
                .to_string()
            )?,
            target_chromium
        );

        Ok(())
    }
}
