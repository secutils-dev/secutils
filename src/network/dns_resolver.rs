use futures::future::BoxFuture;
use hickory_resolver::{
    TokioResolver,
    config::{GOOGLE, ResolverConfig, ResolverOpts},
    lookup_ip::LookupIp,
    net::NetError,
};
use std::time::Duration;

/// Hard upper bound on a single lookup, above hickory's own retry budget (5s timeout x 2 attempts). hickory has been
/// observed to wedge internally after long uptimes, leaving every lookup pending forever, and its own timeouts don't
/// cover that.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(15);

async fn lookup_with_timeout(
    lookup: impl Future<Output = Result<LookupIp, NetError>>,
    timeout: Duration,
) -> Result<LookupIp, NetError> {
    tokio::time::timeout(timeout, lookup)
        .await
        .unwrap_or(Err(NetError::Timeout))
}

/// Trait describing a facade for a DNS resolver.
pub trait DnsResolver: Sync + Send + 'static {
    fn lookup_ip<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<LookupIp, NetError>>;
}

/// A wrapper around `TokioResolver` from `hickory-resolver`.
#[derive(Clone)]
pub struct TokioDnsResolver {
    inner: TokioResolver,
}

impl TokioDnsResolver {
    pub fn create() -> anyhow::Result<Self> {
        Ok(Self {
            inner: TokioResolver::builder_with_config(
                ResolverConfig::udp_and_tcp(&GOOGLE),
                Default::default(),
            )
            .with_options(ResolverOpts::default())
            .build()?,
        })
    }
}

impl DnsResolver for TokioDnsResolver {
    fn lookup_ip<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<LookupIp, NetError>> {
        Box::pin(lookup_with_timeout(
            self.inner.lookup_ip(name),
            LOOKUP_TIMEOUT,
        ))
    }
}

#[cfg(test)]
pub mod tests {
    use super::lookup_with_timeout;
    use crate::network::{DnsResolver, TokioDnsResolver};
    use futures::future::BoxFuture;
    use hickory_resolver::{
        lookup::Lookup,
        lookup_ip::LookupIp,
        net::NetError,
        proto::{
            op::Query,
            rr::{Name, Record, RecordType},
        },
    };

    #[derive(Clone)]
    pub struct MockResolver<const N: usize = 0> {
        records: [Record; N],
        error: Option<NetError>,
    }

    impl<const N: usize> DnsResolver for MockResolver<N> {
        fn lookup_ip<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<LookupIp, NetError>> {
            Box::pin(futures::future::ready(if let Some(err) = &self.error {
                Err(err.clone())
            } else {
                Ok(LookupIp::from(Lookup::new_with_max_ttl(
                    Query::query(Name::new(), RecordType::A),
                    self.records.clone(),
                )))
            }))
        }
    }

    impl MockResolver {
        pub fn new() -> Self {
            MockResolver {
                records: [],
                error: None,
            }
        }
    }

    impl MockResolver {
        pub fn new_with_records<const N: usize>(records: Vec<Record>) -> MockResolver<N> {
            MockResolver {
                records: records.try_into().unwrap(),
                error: None,
            }
        }

        pub fn new_with_error(err: NetError) -> MockResolver<0> {
            MockResolver {
                records: [],
                error: Some(err),
            }
        }
    }

    /// Companion to the test above: actually exercise `create()` end-to-end and assert
    /// that lookups against a non-existent domain do **not** fail with the empty-
    /// `name_servers` error mode. We do not assert success because CI/sandbox network
    /// reachability is out of our control, but the "no connections available" sentinel
    /// is produced locally by hickory before any network I/O and so is detectable
    /// regardless of outbound connectivity.
    #[tokio::test]
    async fn create_yields_resolver_that_does_not_short_circuit_on_empty_name_servers() {
        let resolver = TokioDnsResolver::create().expect("TokioDnsResolver should build");
        // The lookup itself may succeed (NXDOMAIN response) or fail with a network error
        // depending on outbound DNS reachability of the test environment, neither of
        // which we want to assert on. What we *do* assert is that the failure mode
        // (if any) is not the `no connections available` sentinel that hickory raises
        // synchronously, before any network I/O, when `name_servers` is empty.
        if let Err(err) = resolver
            .lookup_ip("nonexistent-secutils-regression-guard.example.")
            .await
        {
            assert!(
                !err.to_string().contains("no connections available"),
                "DNS resolver appears to have no configured name servers: {err}"
            );
        }
    }

    #[tokio::test]
    async fn lookup_with_timeout_fails_a_lookup_that_never_completes() {
        let result =
            lookup_with_timeout(std::future::pending(), std::time::Duration::from_millis(10)).await;
        assert!(matches!(result, Err(NetError::Timeout)));
    }
}
