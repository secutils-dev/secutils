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
}
