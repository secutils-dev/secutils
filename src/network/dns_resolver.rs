use futures::future::BoxFuture;
use hickory_resolver::{
    TokioResolver,
    config::{GOOGLE, ResolverConfig, ResolverOpts},
    lookup_ip::LookupIp,
    net::NetError,
};
use std::{sync::OnceLock, time::Duration};
use tokio::{runtime::Runtime, task::JoinHandle};

/// Hard upper bound on a single lookup, including hickory's retries and background work.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(15);

/// DNS exchanges are shared across callers, but JS workers park their current-thread runtimes
/// between scripts and can retire them. Keep all resolver I/O and background tasks on one
/// continuously driven, process-lifetime runtime instead of the runtime of the first caller.
fn dns_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("dns-resolver")
            .enable_all()
            .build()
            .expect("Failed to build DNS resolver runtime")
    })
}

/// Dropping a cancelled caller also cancels its lookup rather than detaching the spawned task.
struct LookupTask(JoinHandle<Result<LookupIp, NetError>>);

impl Drop for LookupTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

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
        let resolver = self.inner.clone();
        let name = name.to_owned();
        Box::pin(async move {
            let mut task = LookupTask(dns_runtime().spawn(async move {
                lookup_with_timeout(resolver.lookup_ip(name), LOOKUP_TIMEOUT).await
            }));
            (&mut task.0)
                .await
                .map_err(|err| NetError::from(format!("DNS lookup task failed: {err}")))?
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::{LookupTask, dns_runtime, lookup_with_timeout};
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_resolver_keeps_working_when_first_caller_runtime_is_idle() {
        use hickory_resolver::{
            TokioResolver,
            config::{NameServerConfig, ResolverConfig},
            proto::{
                op::{Message, MessageType},
                rr::{RData, rdata::A},
            },
        };
        use std::{net::Ipv4Addr, time::Duration};

        // A local DNS server removes external DNS, caching, and firewall variability.
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut buffer = [0; 512];
            loop {
                let (length, peer) = socket.recv_from(&mut buffer).await.unwrap();
                let request = Message::from_vec(&buffer[..length]).unwrap();
                let mut response = Message::new(request.id, MessageType::Response, request.op_code);
                response.metadata.recursion_desired = true;
                response.metadata.recursion_available = true;
                response.queries = request.queries.clone();
                for query in &request.queries {
                    if query.query_type() == RecordType::A {
                        response.answers.push(Record::from_rdata(
                            query.name().clone(),
                            0,
                            RData::A(A(Ipv4Addr::new(132, 226, 202, 19))),
                        ));
                    }
                }
                socket
                    .send_to(&response.to_vec().unwrap(), peer)
                    .await
                    .unwrap();
            }
        });

        let result = tokio::task::spawn_blocking(move || {
            let mut nameserver = NameServerConfig::udp(address.ip());
            nameserver.connections[0].port = address.port();
            let resolver = TokioDnsResolver {
                inner: TokioResolver::builder_with_config(
                    ResolverConfig::from_name_servers(vec![nameserver]),
                    Default::default(),
                )
                .build()
                .unwrap(),
            };
            let first = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let second = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            first
                .block_on(resolver.lookup_ip("first.example."))
                .unwrap();
            // Keep the first runtime alive but stop driving it, just like an idle JS worker.
            second.block_on(lookup_with_timeout(
                resolver.lookup_ip("second.example."),
                Duration::from_secs(1),
            ))
        })
        .await
        .unwrap();
        server.abort();
        assert!(
            result.is_ok(),
            "lookup depended on an idle caller runtime: {result:?}"
        );
    }

    #[tokio::test]
    async fn dropping_lookup_task_cancels_its_work() {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
        struct NotifyOnDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyOnDrop {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }
        let task = LookupTask(dns_runtime().spawn(async move {
            let _guard = NotifyOnDrop(Some(dropped_tx));
            let _ = started_tx.send(());
            std::future::pending().await
        }));
        started_rx.await.unwrap();
        drop(task);
        tokio::time::timeout(std::time::Duration::from_secs(1), dropped_rx)
            .await
            .unwrap()
            .unwrap();
    }
}
