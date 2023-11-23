use crate::utils::{ResponderMethod, ResponderSettings};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RespondersCreateParams {
    pub name: String,
    /// Path of the responder.
    pub path: String,
    /// HTTP method of the responder.
    pub method: ResponderMethod,
    // Miscellaneous responder settings.
    pub settings: ResponderSettings,
}

#[cfg(test)]
mod tests {
    use crate::utils::{ResponderMethod, ResponderSettings, RespondersCreateParams};
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<RespondersCreateParams>(
                r#"
{
    "name": "res",
    "path": "/",
    "method": "GET",
    "settings": {
        "requestsToTrack": 10,
        "statusCode": 302,
        "body": "some-body",
        "headers": [["key", "value"], ["key2", "value2"]],
        "delay": 123
    }
}
          "#
            )?,
            RespondersCreateParams {
                name: "res".to_string(),
                path: "/".to_string(),
                method: ResponderMethod::Get,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    delay: Duration::from_millis(123),
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersCreateParams>(
                r#"
{
    "name": "res",
    "path": "/",
    "method": "GET",
    "settings": {
        "statusCode": 302,
        "delay": 123
    }
}
          "#
            )?,
            RespondersCreateParams {
                name: "res".to_string(),
                path: "/".to_string(),
                method: ResponderMethod::Get,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 302,
                    body: None,
                    headers: None,
                    delay: Duration::from_millis(123),
                }
            }
        );

        Ok(())
    }
}
