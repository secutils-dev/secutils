use crate::utils::{ResponderMethod, ResponderSettings};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RespondersUpdateParams {
    pub name: Option<String>,
    /// Path of the responder.
    pub path: Option<String>,
    /// HTTP method of the responder.
    pub method: Option<ResponderMethod>,
    // Miscellaneous responder settings.
    pub settings: Option<ResponderSettings>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{ResponderMethod, ResponderSettings, RespondersUpdateParams};
    use std::time::Duration;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
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
            RespondersUpdateParams {
                name: Some("res".to_string()),
                path: Some("/".to_string()),
                method: Some(ResponderMethod::Get),
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    delay: Duration::from_millis(123),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
    "path": "/",
    "method": "POST",
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
            RespondersUpdateParams {
                name: None,
                path: Some("/".to_string()),
                method: Some(ResponderMethod::Post),
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    delay: Duration::from_millis(123),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
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
            RespondersUpdateParams {
                name: None,
                path: None,
                method: Some(ResponderMethod::Get),
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    delay: Duration::from_millis(123),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
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
            RespondersUpdateParams {
                name: None,
                path: None,
                method: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    delay: Duration::from_millis(123),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
    "settings": {
        "statusCode": 302,
        "delay": 123
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: None,
                path: None,
                method: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 0,
                    status_code: 302,
                    body: None,
                    headers: None,
                    delay: Duration::from_millis(123),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(r#"{}"#)?,
            RespondersUpdateParams {
                name: None,
                path: None,
                method: None,
                settings: None
            }
        );

        Ok(())
    }
}
