mod dns_resolver;
mod ip_addr_ext;

pub use self::{
    dns_resolver::{DnsResolver, TokioDnsResolver},
    ip_addr_ext::IpAddrExt,
};

#[cfg(test)]
pub use self::dns_resolver::tests;

/// Network utilities.
#[derive(Clone)]
pub struct Network<DR: DnsResolver> {
    pub resolver: DR,
}

impl<DR: DnsResolver> Network<DR> {
    /// Creates a new `Network` instance.
    pub fn new(resolver: DR) -> Self {
        Self { resolver }
    }
}
