use crate::utils::webhooks::{ResponderLocation, ResponderMethod, ResponderSettings};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RespondersCreateParams {
    pub name: String,
    /// Location of the responder.
    pub location: ResponderLocation,
    /// HTTP method of the responder.
    pub method: ResponderMethod,
    /// Indicates whether the responder is enabled.
    pub enabled: bool,
    // Miscellaneous responder settings.
    pub settings: ResponderSettings,
    /// Tag IDs to assign to this responder.
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use crate::{
        users::SecretsAccess,
        utils::webhooks::{
            ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
            api_ext::RespondersCreateParams,
        },
    };
    use uuid::uuid;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<RespondersCreateParams>(
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
    },
    "tagIds": ["00000000-0000-0000-0000-000000000001"]
}
          "#
            )?,
            RespondersCreateParams {
                name: "res".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 302,
                    body: Some("some-body".to_string()),
                    headers: Some(vec![
                        ("key".to_string(), "value".to_string()),
                        ("key2".to_string(), "value2".to_string())
                    ]),
                    script: Some("return { body: `custom body` };".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![uuid!("00000000-0000-0000-0000-000000000001")],
            }
        );

        assert_eq!(
            serde_json::from_str::<RespondersCreateParams>(
                r#"
{
    "name": "res",
    "location": {
        "subdomainPrefix": "sub",
        "path": "/path",
        "pathType": "^"
    },
    "method": "GET",
    "enabled": false,
    "settings": {
        "statusCode": 302
    }
}
          "#
            )?,
            RespondersCreateParams {
                name: "res".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Prefix,
                    path: "/path".to_string(),
                    subdomain_prefix: Some("sub".to_string())
                },
                method: ResponderMethod::Get,
                enabled: false,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 302,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            }
        );

        Ok(())
    }
}
