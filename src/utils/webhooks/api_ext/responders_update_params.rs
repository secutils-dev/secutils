use crate::utils::webhooks::{ResponderLocation, ResponderMethod, ResponderSettings};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RespondersUpdateParams {
    pub name: Option<String>,
    /// Location of the responder.
    pub location: Option<ResponderLocation>,
    /// HTTP method of the responder.
    pub method: Option<ResponderMethod>,
    /// Whether the responder is enabled.
    pub enabled: Option<bool>,
    // Miscellaneous responder settings.
    pub settings: Option<ResponderSettings>,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::{
        api_ext::RespondersUpdateParams, ResponderLocation, ResponderMethod, ResponderPathType,
        ResponderSettings,
    };

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
    "name": "res",
    "location": {
        "path": "/",
        "pathType": "="
    },
    "method": "GET",
    "enabled": true,
    "settings": {
        "requestsToTrack": 10,
        "statusCode": 302,
        "body": "some-body",
        "headers": [["key", "value"], ["key2", "value2"]],
        "script": "return { body: `custom body` };"
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: Some("res".to_string()),
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain: None
                }),
                method: Some(ResponderMethod::Get),
                enabled: Some(true),
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    script: Some("return { body: `custom body` };".to_string()),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
    "name": "res",
    "location": {
        "subdomain": "sub",
        "path": "/path",
        "pathType": "^"
    },
    "method": "GET",
    "settings": {
        "requestsToTrack": 10,
        "statusCode": 302,
        "body": "some-body",
        "headers": [["key", "value"], ["key2", "value2"]],
        "script": "return { body: `custom body` };"
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: Some("res".to_string()),
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Prefix,
                    path: "/path".to_string(),
                    subdomain: Some("sub".to_string())
                }),
                method: Some(ResponderMethod::Get),
                enabled: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    script: Some("return { body: `custom body` };".to_string()),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
    "location": {
        "path": "/path",
        "pathType": "="
    },
    "method": "POST",
    "settings": {
        "requestsToTrack": 10,
        "statusCode": 302,
        "body": "some-body",
        "headers": [["key", "value"], ["key2", "value2"]],
        "script": "return { body: `custom body` };"
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: None,
                location: Some(ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/path".to_string(),
                    subdomain: None
                }),
                method: Some(ResponderMethod::Post),
                enabled: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    script: Some("return { body: `custom body` };".to_string()),
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
        "script": "return { body: `custom body` };"
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: None,
                location: None,
                method: Some(ResponderMethod::Get),
                enabled: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    script: Some("return { body: `custom body` };".to_string()),
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
        "script": "return { body: `custom body` };"
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    script: Some("return { body: `custom body` };".to_string()),
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(
                r#"
{
    "settings": {
        "statusCode": 302
    }
}
          "#
            )?,
            RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: Some(ResponderSettings {
                    requests_to_track: 0,
                    status_code: 302,
                    body: None,
                    headers: None,
                    script: None
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersUpdateParams>(r#"{}"#)?,
            RespondersUpdateParams {
                name: None,
                location: None,
                method: None,
                enabled: None,
                settings: None
            }
        );

        Ok(())
    }
}
