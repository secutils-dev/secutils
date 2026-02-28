use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use url::Url;

/// Flattened single-request API tracker target. Converted to/from Retrack's
/// `TrackerTarget::Api(ApiTarget { requests: [TargetRequest], ... })` internally.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiTrackerTarget {
    /// URL of the API endpoint.
    pub url: Url,
    /// HTTP method (defaults to GET).
    pub method: Option<String>,
    /// Optional request headers.
    pub headers: Option<HashMap<String, String>>,
    /// Optional request body (JSON).
    pub body: Option<JsonValue>,
    /// Expected media type of the response (defaults to application/json).
    pub media_type: Option<String>,
    /// HTTP status codes considered successful (defaults to 2xx).
    pub accept_statuses: Option<Vec<u16>>,
    /// Whether to accept invalid TLS certificates.
    #[serde(default)]
    pub accept_invalid_certificates: bool,
    /// Optional Deno script to configure the request dynamically.
    pub configurator: Option<String>,
    /// Optional Deno script to extract data from the response.
    pub extractor: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::ApiTrackerTarget;
    use serde_json::json;

    #[test]
    fn deserialization_minimal() -> anyhow::Result<()> {
        let target: ApiTrackerTarget =
            serde_json::from_value(json!({ "url": "https://api.example.com/data" }))?;
        assert_eq!(target.url.as_str(), "https://api.example.com/data");
        assert!(target.method.is_none());
        assert!(target.headers.is_none());
        assert!(target.body.is_none());
        assert!(!target.accept_invalid_certificates);
        assert!(target.configurator.is_none());
        assert!(target.extractor.is_none());
        Ok(())
    }

    #[test]
    fn deserialization_full() -> anyhow::Result<()> {
        let target: ApiTrackerTarget = serde_json::from_value(json!({
            "url": "https://api.example.com/data",
            "method": "POST",
            "headers": { "Authorization": "Bearer tok" },
            "body": { "key": "value" },
            "mediaType": "application/json",
            "acceptStatuses": [200, 201],
            "acceptInvalidCertificates": true,
            "configurator": "(() => ({ requests: context.requests }))()",
            "extractor": "(() => ({ body: Deno.core.encode(JSON.stringify({})) }))()"
        }))?;
        assert_eq!(target.method.as_deref(), Some("POST"));
        assert_eq!(
            target
                .headers
                .as_ref()
                .unwrap()
                .get("Authorization")
                .unwrap(),
            "Bearer tok"
        );
        assert_eq!(target.body, Some(json!({ "key": "value" })));
        assert_eq!(target.media_type.as_deref(), Some("application/json"));
        assert_eq!(target.accept_statuses, Some(vec![200, 201]));
        assert!(target.accept_invalid_certificates);
        assert!(target.configurator.is_some());
        assert!(target.extractor.is_some());
        Ok(())
    }
}
