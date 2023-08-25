mod dns_resolver;
mod email_transport;
mod ip_addr_ext;

pub use self::{
    dns_resolver::{DnsResolver, TokioDnsResolver},
    email_transport::{EmailTransport, EmailTransportError},
    ip_addr_ext::IpAddrExt,
};

#[cfg(test)]
pub use self::dns_resolver::tests;

/// Network utilities.
#[derive(Clone)]
pub struct Network<DR: DnsResolver, ET: EmailTransport> {
    pub resolver: DR,
    pub email_transport: ET,
}

impl<DR: DnsResolver, ET: EmailTransport> Network<DR, ET> {
    /// Creates a new `Network` instance.
    pub fn new(resolver: DR, email_transport: ET) -> Self {
        Self {
            resolver,
            email_transport,
        }
    }
}
