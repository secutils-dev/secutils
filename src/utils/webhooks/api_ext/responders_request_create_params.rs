use crate::utils::webhooks::ResponderRequestHeaders;
use std::{borrow::Cow, net::SocketAddr};

#[derive(Debug, PartialEq, Eq)]
pub struct RespondersRequestCreateParams<'a> {
    /// An internet socket address of the client that made the request.
    pub client_address: Option<SocketAddr>,
    /// HTTP method of the request.
    pub method: Cow<'a, str>,
    /// HTTP headers of the request.
    pub headers: Option<ResponderRequestHeaders<'a>>,
    /// HTTP path of the request + query string.
    pub url: Cow<'a, str>,
    /// HTTP body of the request.
    pub body: Option<Cow<'a, [u8]>>,
    /// Total server-side processing time in milliseconds.
    pub duration_ms: Option<u32>,
    /// HTTP status code of the tracked response.
    pub response_status_code: Option<u16>,
    /// HTTP headers of the tracked response.
    pub response_headers: Option<ResponderRequestHeaders<'a>>,
    /// HTTP body of the tracked response.
    pub response_body: Option<Cow<'a, [u8]>>,
}
