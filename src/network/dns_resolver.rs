use futures::future::BoxFuture;
use hickory_resolver::{
    TokioResolver,
    config::{ResolverConfig, ResolverOpts},
    lookup_ip::LookupIp,
    net::NetError,
};

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
                ResolverConfig::default(),
                Default::default(),
            )
            .with_options(ResolverOpts::default())
            .build()?,
        })
    }
}

impl DnsResolver for TokioDnsResolver {
    fn lookup_ip<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<LookupIp, NetError>> {
        Box::pin(self.inner.lookup_ip(name))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::network::DnsResolver;
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
}
