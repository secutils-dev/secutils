use crate::utils::web_scraping::api_trackers::ApiTrackerTarget;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiTrackerTestParams {
    pub target: ApiTrackerTarget,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiTrackerTestResult {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub latency_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::ApiTrackerTestParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let params: ApiTrackerTestParams =
            serde_json::from_str(r#"{ "target": { "url": "https://api.example.com/data" } }"#)?;
        assert_eq!(params.target.url.as_str(), "https://api.example.com/data");
        assert!(params.target.method.is_none());

        let params: ApiTrackerTestParams = serde_json::from_str(
            r#"{ "target": { "url": "https://api.example.com/data", "method": "POST", "headers": { "Content-Type": "application/json" }, "body": {"key": "value"} } }"#,
        )?;
        assert_eq!(params.target.method.as_deref(), Some("POST"));
        assert!(params.target.headers.is_some());
        assert!(params.target.body.is_some());

        Ok(())
    }
}
