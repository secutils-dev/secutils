use super::AutoResponderMethod;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoResponder {
    #[serde(rename = "a")]
    pub alias: String,
    #[serde(rename = "m")]
    pub method: AutoResponderMethod,
    #[serde(rename = "t", skip_serializing_if = "Option::is_none")]
    pub requests_to_track: Option<usize>,
    #[serde(rename = "s")]
    pub status_code: u16,
    #[serde(rename = "b", skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(rename = "h", skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,
    #[serde(rename = "d", skip_serializing_if = "Option::is_none")]
    pub delay: Option<usize>,
}

impl AutoResponder {
    /// Checks whether responder is semantically valid.
    pub fn is_valid(&self) -> bool {
        !self.alias.is_empty() && (100..=999).contains(&self.status_code)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{AutoResponder, AutoResponderMethod};
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(AutoResponder {
            alias: "some-alias".to_string(),
            method: AutoResponderMethod::Post,
            requests_to_track: None,
            status_code: 123,
            body: None,
            headers: None,
            delay: None
        }, @r###"
        {
          "a": "some-alias",
          "m": "p",
          "s": 123
        }
        "###);

        assert_json_snapshot!(AutoResponder {
            alias: "some-alias".to_string(),
            method: AutoResponderMethod::Post,
             requests_to_track: Some(10),
            status_code: 123,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: Some(1000)
        }, @r###"
        {
          "a": "some-alias",
          "m": "p",
          "t": 10,
          "s": 123,
          "b": "body",
          "h": [
            [
              "key",
              "value"
            ]
          ],
          "d": 1000
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<AutoResponder>(
                &json!({ "a": "some-alias", "m": "p", "s": 123 }).to_string()
            )?,
            AutoResponder {
                alias: "some-alias".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: None,
                status_code: 123,
                body: None,
                headers: None,
                delay: None
            }
        );

        assert_eq!(
            serde_json::from_str::<AutoResponder>(
                &json!({ "a": "some-alias", "m": "p", "t": 10, "s": 123, "b": "body", "h": [["key", "value"]], "d": 1000 }).to_string()
            )?,
            AutoResponder {
                alias: "some-alias".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: Some(10),
                status_code: 123,
                body: Some("body".to_string()),
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: Some(1000)
            }
        );

        Ok(())
    }

    #[test]
    fn properly_check_if_valid() -> anyhow::Result<()> {
        for (alias, is_valid) in [("some-alias", true), ("a", true), ("", false)] {
            assert_eq!(
                AutoResponder {
                    alias: alias.to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: None,
                    status_code: 123,
                    body: None,
                    headers: None,
                    delay: None,
                }
                .is_valid(),
                is_valid
            );
        }

        for (method, is_valid) in [
            (AutoResponderMethod::Get, true),
            (AutoResponderMethod::Connect, true),
            (AutoResponderMethod::Any, true),
        ] {
            assert_eq!(
                AutoResponder {
                    alias: "some-alias".to_string(),
                    method,
                    requests_to_track: None,
                    status_code: 123,
                    body: None,
                    headers: None,
                    delay: None,
                }
                .is_valid(),
                is_valid
            );
        }

        for (status_code, is_valid) in [
            (100, true),
            (123, true),
            (200, true),
            (500, true),
            (650, true),
            (999, true),
            (99, false),
            (1000, false),
            (0, false),
        ] {
            assert_eq!(
                AutoResponder {
                    alias: "some-alias".to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: None,
                    status_code,
                    body: None,
                    headers: None,
                    delay: None,
                }
                .is_valid(),
                is_valid
            );
        }

        assert!(AutoResponder {
            alias: "some-alias".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: Some(10),
            status_code: 123,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: Some(1000)
        }
        .is_valid());

        Ok(())
    }
}
