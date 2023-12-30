use bytes::Bytes;
use serde::Deserialize;
use std::collections::HashMap;

/// Result of the responder script execution.
#[derive(Deserialize, Default, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderScriptResult {
    /// HTTP status code to respond with. If not specified, the default status code of responder is used.
    pub status_code: Option<u16>,
    /// Optional HTTP headers of the response. If not specified, the default headers of responder are used.
    pub headers: Option<HashMap<String, String>>,
    /// Optional HTTP body of the response. If not specified, the default body of responder is used.
    pub body: Option<Bytes>,
}

#[cfg(test)]
mod tests {
    use crate::utils::ResponderScriptResult;
    use bytes::Bytes;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderScriptResult>(
                r#"
{
    "body": [1, 2 ,3],
    "statusCode": 300,
    "headers": {
        "one": "two"
    }
}
          "#
            )?,
            ResponderScriptResult {
                headers: Some(
                    [("one".to_string(), "two".to_string())]
                        .into_iter()
                        .collect()
                ),
                status_code: Some(300),
                body: Some(Bytes::from_static(&[1, 2, 3])),
            }
        );

        assert_eq!(
            serde_json::from_str::<ResponderScriptResult>(r#"{}"#)?,
            Default::default()
        );

        Ok(())
    }
}
