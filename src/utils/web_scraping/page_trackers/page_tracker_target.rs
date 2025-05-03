use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageTrackerTarget {
    /// A custom script (Playwright scenario) to extract data from the page.
    pub extractor: String,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::page_trackers::PageTrackerTarget;
    use serde_json::json;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let target = PageTrackerTarget {
            extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<PageTrackerTarget>(
                &json!({ "extractor": "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }" }).to_string()
            )?,
            target
        );

        Ok(())
    }
}
