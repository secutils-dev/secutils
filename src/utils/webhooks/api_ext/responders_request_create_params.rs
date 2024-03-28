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
}
