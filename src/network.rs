mod dns_resolver;
mod email_transport;
mod ip_addr_ext;

pub use self::{
    dns_resolver::{DnsResolver, TokioDnsResolver},
    email_transport::{EmailTransport, EmailTransportError},
    ip_addr_ext::IpAddrExt,
};
use crate::config::{Config, SECUTILS_USER_AGENT};
use anyhow::Context;
use lettre::{
    AsyncSmtpTransport, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest_tracing::{SpanBackendWithUrl, TracingMiddleware};
use std::{net::IpAddr, str::FromStr};
use tracing::error;
use url::{Host, Url};

/// Network utilities.
#[derive(Clone)]
pub struct Network<DR: DnsResolver, ET: EmailTransport> {
    pub resolver: DR,
    pub email_transport: ET,
    pub http_client: ClientWithMiddleware,
}

impl<DR: DnsResolver, ET: EmailTransport> Network<DR, ET> {
    /// Creates a new `Network` instance.
    pub fn new(resolver: DR, email_transport: ET, http_client: ClientWithMiddleware) -> Self {
        Self {
            resolver,
            email_transport,
            http_client,
        }
    }

    /// Checks if the provided URL is a publicly accessible web URL.
    pub async fn is_public_web_url(&self, url: &Url) -> bool {
        if url.scheme() != "http" && url.scheme() != "https" {
            return false;
        }

        // Checks if the specific hostname is a domain and public (not pointing to the local network).
        match url.host() {
            Some(Host::Domain(domain)) => match self.resolver.lookup_ip(domain).await {
                Ok(lookup) => lookup.iter().all(|ip| IpAddrExt::is_global(&ip)),
                Err(err) => {
                    error!("Cannot resolve domain ({domain}) to IP: {err}");
                    false
                }
            },
            Some(Host::Ipv4(ip)) => IpAddrExt::is_global(&IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => IpAddrExt::is_global(&IpAddr::V6(ip)),
            None => false,
        }
    }
}

impl Network<TokioDnsResolver, AsyncSmtpTransport<Tokio1Executor>> {
    pub fn create(config: &Config) -> anyhow::Result<Self> {
        let email_transport = if let Some(ref smtp_config) = config.smtp {
            if let Some(ref catch_all_config) = smtp_config.catch_all {
                Mailbox::from_str(catch_all_config.recipient.as_str())
                    .with_context(|| "Cannot parse SMTP catch-all recipient.")?;
            }

            AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_config.address)?
                .credentials(Credentials::new(
                    smtp_config.username.clone(),
                    smtp_config.password.clone(),
                ))
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost()
        };

        let http_client_builder = ClientBuilder::new(
            Client::builder()
                .user_agent(SECUTILS_USER_AGENT)
                .pool_idle_timeout(Some(config.http.client.pool_idle_timeout))
                .timeout(config.http.client.timeout)
                .connection_verbose(config.http.client.verbose)
                .build()
                .with_context(|| "Cannot build HTTP client.")?,
        )
        .with(TracingMiddleware::<SpanBackendWithUrl>::new());

        let http_client = if config.http.client.max_retries > 0 {
            http_client_builder
                .with(RetryTransientMiddleware::new_with_policy(
                    ExponentialBackoff::builder()
                        .build_with_max_retries(config.http.client.max_retries),
                ))
                .build()
        } else {
            http_client_builder.build()
        };

        Ok(Self::new(
            TokioDnsResolver::create(),
            email_transport,
            http_client,
        ))
    }
}

#[cfg(test)]
pub mod tests {
    use super::Network;
    use lettre::transport::stub::AsyncStubTransport;
    use reqwest::Client;
    use std::net::Ipv4Addr;
    use trust_dns_resolver::{
        Name,
        error::{ResolveError, ResolveErrorKind},
        proto::rr::{RData, Record, rdata::A},
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
            Client::new().into(),
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
            Client::new().into(),
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
            Client::new().into(),
        );
        assert!(!broken_network.is_public_web_url(&url).await);

        Ok(())
    }

    #[tokio::test]
    async fn correctly_checks_public_ips() -> anyhow::Result<()> {
        let network = Network::new(
            MockResolver::new(),
            AsyncStubTransport::new_ok(),
            Client::new().into(),
        );
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
