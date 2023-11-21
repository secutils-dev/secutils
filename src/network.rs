mod dns_resolver;
mod email_transport;
mod ip_addr_ext;

pub use self::{
    dns_resolver::{DnsResolver, TokioDnsResolver},
    email_transport::{EmailTransport, EmailTransportError},
    ip_addr_ext::IpAddrExt,
};
use std::net::IpAddr;
use url::{Host, Url};

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

    /// Checks if provided URL is a publicly accessible web URL.
    pub async fn is_public_web_url(&self, url: &Url) -> bool {
        if url.scheme() != "http" && url.scheme() != "https" {
            return false;
        }

        // Checks if the specific hostname is a domain and public (not pointing to the local network).
        match url.host() {
            Some(Host::Domain(domain)) => match self.resolver.lookup_ip(domain).await {
                Ok(lookup) => lookup.iter().all(|ip| IpAddrExt::is_global(&ip)),
                Err(err) => {
                    log::error!("Cannot resolve domain ({domain}) to IP: {err}");
                    false
                }
            },
            Some(Host::Ipv4(ip)) => IpAddrExt::is_global(&IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => IpAddrExt::is_global(&IpAddr::V6(ip)),
            None => false,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::Network;
    use lettre::transport::stub::AsyncStubTransport;
    use std::net::Ipv4Addr;
    use trust_dns_resolver::{
        error::{ResolveError, ResolveErrorKind},
        proto::rr::{rdata::A, RData, Record},
        Name,
    };
    use url::Url;

    pub use super::dns_resolver::tests::*;

    #[tokio::test]
    async fn correctly_checks_public_web_urls() -> anyhow::Result<()> {
        let public_network = Network::new(
            MockResolver::new_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]),
            AsyncStubTransport::new_ok(),
        );

        // Only `http` and `https` should be supported.
        for (protocol, is_supported) in [
            ("ftp", false),
            ("wss", false),
            ("http", true),
            ("https", true),
        ] {
            let url = Url::parse(&format!("{}://secutils.dev/my-page", protocol))?;
            assert_eq!(public_network.is_public_web_url(&url).await, is_supported);
        }

        // Hosts that resolve to local IPs aren't supported.
        let url = Url::parse("https://secutils.dev/my-page")?;
        let local_network = Network::new(
            MockResolver::new_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]),
            AsyncStubTransport::new_ok(),
        );
        for (network, is_supported) in [(public_network, true), (local_network, false)] {
            assert_eq!(network.is_public_web_url(&url).await, is_supported);
        }

        // Hosts that fail to resolve aren't supported and gracefully handled.
        let broken_network = Network::new(
            MockResolver::new_with_error(ResolveError::from(ResolveErrorKind::Message(
                "can not lookup IPs",
            ))),
            AsyncStubTransport::new_ok(),
        );
        assert!(!broken_network.is_public_web_url(&url).await);

        Ok(())
    }

    #[tokio::test]
    async fn correctly_checks_public_ips() -> anyhow::Result<()> {
        let network = Network::new(MockResolver::new(), AsyncStubTransport::new_ok());
        for (ip, is_supported) in [
            ("127.0.0.1", false),
            ("10.254.0.0", false),
            ("192.168.10.65", false),
            ("172.16.10.65", false),
            ("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]", false),
            ("[::1]", false),
            ("217.88.39.143", true),
            ("[2001:1234:abcd:5678:0221:2fff:feb5:6e10]", true),
        ] {
            let url = Url::parse(&format!("http://{}/my-page", ip))?;
            assert_eq!(network.is_public_web_url(&url).await, is_supported);
        }

        Ok(())
    }
}
