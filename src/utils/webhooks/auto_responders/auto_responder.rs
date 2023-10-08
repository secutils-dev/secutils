use crate::utils::{utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, AutoResponderMethod};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoResponder {
    #[serde(rename = "p")]
    pub path: String,
    #[serde(rename = "m")]
    pub method: AutoResponderMethod,
    #[serde(rename = "t")]
    pub requests_to_track: usize,
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
        Self::is_path_valid(&self.path)
            && (100..=999).contains(&self.status_code)
            && (0..=100).contains(&self.requests_to_track)
    }

    /// Checks whether responder path is valid.
    pub fn is_path_valid(path: &str) -> bool {
        path.starts_with('/')
            && (path.len() == 1 || !path.ends_with('/'))
            && path.len() <= MAX_UTILS_ENTITY_NAME_LENGTH
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, AutoResponder, AutoResponderMethod,
    };
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(AutoResponder {
            path: "/some-name".to_string(),
            method: AutoResponderMethod::Post,
            requests_to_track: 10,
            status_code: 123,
            body: None,
            headers: None,
            delay: None
        }, @r###"
        {
          "p": "/some-name",
          "m": "p",
          "t": 10,
          "s": 123
        }
        "###);

        assert_json_snapshot!(AutoResponder {
            path: "/some-name".to_string(),
            method: AutoResponderMethod::Post,
            requests_to_track: 10,
            status_code: 123,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: Some(1000)
        }, @r###"
        {
          "p": "/some-name",
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
                &json!({ "p": "/some-name", "m": "p", "t": 10, "s": 123 }).to_string()
            )?,
            AutoResponder {
                path: "/some-name".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: 10,
                status_code: 123,
                body: None,
                headers: None,
                delay: None
            }
        );

        assert_eq!(
            serde_json::from_str::<AutoResponder>(
                &json!({ "p": "/some-name", "m": "p", "t": 10, "s": 123, "b": "body", "h": [["key", "value"]], "d": 1000 }).to_string()
            )?,
            AutoResponder {
                path: "/some-name".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: 10,
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
        for (path, is_valid) in [
            ("/some-name", true),
            ("/n", true),
            ("/", true),
            ("", false),
            ("/n/", false),
            (&"/n".repeat(MAX_UTILS_ENTITY_NAME_LENGTH / 2), true),
            (&"/n".repeat(MAX_UTILS_ENTITY_NAME_LENGTH / 2 + 1), false),
        ] {
            assert_eq!(
                AutoResponder {
                    path: path.to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: 10,
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
                    path: "/some-name".to_string(),
                    method,
                    requests_to_track: 10,
                    status_code: 123,
                    body: None,
                    headers: None,
                    delay: None,
                }
                .is_valid(),
                is_valid
            );
        }

        for (requests_to_track, is_valid) in
            [(0, true), (1, true), (10, true), (100, true), (101, false)]
        {
            assert_eq!(
                AutoResponder {
                    path: "/some-name".to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track,
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
                    path: "/some-name".to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: 10,
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
            path: "/some-name".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 10,
            status_code: 123,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: Some(1000)
        }
        .is_valid());

        Ok(())
    }

    #[test]
    fn properly_check_if_path_valid() -> anyhow::Result<()> {
        for (path, is_valid) in [
            ("/some-name", true),
            ("/n", true),
            ("/", true),
            ("", false),
            ("/n/", false),
            (&"/n".repeat(MAX_UTILS_ENTITY_NAME_LENGTH / 2), true),
            (&"/n".repeat(MAX_UTILS_ENTITY_NAME_LENGTH / 2 + 1), false),
        ] {
            assert_eq!(AutoResponder::is_path_valid(path), is_valid);
        }

        Ok(())
    }
}
