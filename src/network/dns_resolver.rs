use futures::future::BoxFuture;
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    error::ResolveError,
    lookup_ip::LookupIp,
    TokioAsyncResolver,
};

/// Trait describing a facade for a `AsyncResolver` from `trust-dns-resolver`.
pub trait DnsResolver: Sync + Send + 'static {
    fn lookup_ip<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<LookupIp, ResolveError>>;
}

/// A wrapper around `TokioAsyncResolver` from `trust-dns-resolver`.
#[derive(Clone)]
pub struct TokioDnsResolver {
    inner: TokioAsyncResolver,
}

impl TokioDnsResolver {
    pub fn create() -> anyhow::Result<Self> {
        Ok(Self {
            inner: TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())?,
        })
    }
}

impl DnsResolver for TokioDnsResolver {
    fn lookup_ip<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<LookupIp, ResolveError>> {
        Box::pin(self.inner.lookup_ip(name))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::network::DnsResolver;
    use futures::future::BoxFuture;
    use std::sync::Arc;
    use trust_dns_resolver::{
        error::ResolveError,
        lookup::Lookup,
        lookup_ip::LookupIp,
        proto::{
            op::Query,
            rr::{Record, RecordType},
        },
        Name,
    };

    #[derive(Clone)]
    pub struct MockResolver<const N: usize = 0> {
        records: [Record; N],
    }

    impl<const N: usize> DnsResolver for MockResolver<N> {
        fn lookup_ip<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<LookupIp, ResolveError>> {
            Box::pin(futures::future::ready(Ok(LookupIp::from(
                Lookup::new_with_max_ttl(
                    Query::query(Name::new(), RecordType::A),
                    Arc::new(self.records.clone()),
                ),
            ))))
        }
    }

    impl MockResolver {
        pub fn new() -> Self {
            MockResolver { records: [] }
        }
    }

    impl MockResolver {
        pub fn new_with_records<const N: usize>(records: Vec<Record>) -> MockResolver<N> {
            MockResolver {
                records: records.try_into().unwrap(),
            }
        }
    }
}
