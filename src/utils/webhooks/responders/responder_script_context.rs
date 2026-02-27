use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr};

/// Context available to the responder scripts through global `context` variable.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderScriptContext<'a> {
    /// An internet socket address of the client that made the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_address: Option<SocketAddr>,
    /// HTTP method of the received request.
    pub method: &'a str,
    /// HTTP headers of the received request.
    pub headers: HashMap<&'a str, &'a str>,
    /// HTTP path of the received request.
    pub path: &'a str,
    /// Parsed query string of the received request.
    pub query: HashMap<&'a str, &'a str>,
    /// HTTP body of the received request.
    pub body: &'a [u8],
    /// User secrets (decrypted key-value pairs).
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub secrets: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::ResponderScriptContext;
    use insta::assert_json_snapshot;
    use std::collections::HashMap;

    #[test]
    fn serialization_without_secrets() {
        let ctx = ResponderScriptContext {
            client_address: None,
            method: "GET",
            headers: HashMap::new(),
            path: "/test",
            query: HashMap::new(),
            body: b"",
            secrets: HashMap::new(),
        };
        assert_json_snapshot!(ctx, @r###"
        {
          "method": "GET",
          "headers": {},
          "path": "/test",
          "query": {},
          "body": []
        }
        "###);
    }

    #[test]
    fn serialization_with_secrets() {
        let mut secrets = HashMap::new();
        secrets.insert("API_KEY".to_string(), "sk-123".to_string());
        let ctx = ResponderScriptContext {
            client_address: None,
            method: "POST",
            headers: HashMap::new(),
            path: "/api",
            query: HashMap::new(),
            body: b"hello",
            secrets,
        };
        let json = serde_json::to_value(&ctx).unwrap();
        assert!(json.get("secrets").is_some());
        assert_eq!(json["secrets"]["API_KEY"], "sk-123");
    }
}
