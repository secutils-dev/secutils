use crate::utils::webhooks::ResponderRequestHeaders;
use std::{borrow::Cow, net::IpAddr};

#[derive(Debug, PartialEq, Eq)]
pub struct RespondersRequestCreateParams<'a> {
    /// IP address of the client that made the request.
    pub client_address: Option<IpAddr>,
    /// HTTP method of the request.
    pub method: Cow<'a, str>,
    /// HTTP headers of the request.
    pub headers: Option<ResponderRequestHeaders<'a>>,
    /// HTTP body of the request.
    pub body: Option<Cow<'a, [u8]>>,
}
