use serde::Serialize;
use std::collections::HashMap;

/// Context available to the responder scripts through global `context` variable.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderScriptContext<'a> {
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
}
