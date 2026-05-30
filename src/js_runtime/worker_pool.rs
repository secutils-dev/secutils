//! A pool of long-lived worker threads, each owning a persistent
//! `CurrentThread` tokio runtime + `LocalSet`. Script executions are
//! dispatched round-robin to workers over an unbounded mpsc channel and run
//! *concurrently* within each worker (each task is `spawn_local`-ed), so a
//! script that parks on an async op - e.g. a `secutils.kv.watch` long-poll
//! that can idle for tens of seconds - never blocks the other scripts sharing
//! its worker thread. CPU-bound work is still cooperative: a script doing
//! synchronous V8 work holds the thread until its next await point.
//!
//! This replaces the previous per-call `spawn_blocking` + `new_current_thread`
//! pattern, which paid the full cost of building a fresh tokio runtime (and
//! its I/O driver, which consumes a kqueue/epoll fd) on every invocation.
//!
//! Each task still creates a fresh V8 isolate for strong isolation between
//! scripts; reusing the worker thread and its tokio runtime is what yields
//! the steady-state win. Future work (V8 startup snapshot, isolate pooling)
//! can further reduce per-task cost on top of this foundation.
//!
//! Note: we deliberately keep an unbounded channel here. Back-pressure for
//! user-visible workloads is already applied upstream (e.g., the
//! `max_concurrent_responder_requests` semaphore in the responder handler),
//! and queueing cheap `ScriptTask`s is preferable to blocking the producing
//! future on `mpsc::Sender::send`.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{runtime::Builder, sync::mpsc, task::LocalSet};

/// A boxed, thread-movable closure that, when invoked on a worker thread,
/// yields a (!Send) future performing the actual script work. The future is
/// `!Send` because V8 isolates are tied to the thread that created them;
/// running it inside a `LocalSet` is sufficient.
type TaskBuilder = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + 'static>> + Send>;

/// A unit of work dispatched to a worker. Owns everything it needs, including
/// the oneshot sender for the typed result, so the worker does not need to
/// know anything about the caller's generic parameters.
pub struct ScriptTask {
    pub build: TaskBuilder,
}

impl ScriptTask {
    pub fn new<F>(build: F) -> Self
    where
        F: FnOnce() -> Pin<Box<dyn Future<Output = ()> + 'static>> + Send + 'static,
    {
        Self {
            build: Box::new(build),
        }
    }
}

/// A round-robin pool of worker threads. Each worker has its own tokio
/// `CurrentThread` runtime and `LocalSet`; tasks submitted to a worker run
/// sequentially in FIFO order, so the pool provides up to `workers.len()`-way
/// parallelism across workers.
pub struct WorkerPool {
    workers: Vec<mpsc::UnboundedSender<ScriptTask>>,
    next: AtomicUsize,
}

impl WorkerPool {
    fn new(num_workers: usize) -> Self {
        let num_workers = num_workers.max(1);
        let mut workers = Vec::with_capacity(num_workers);
        for idx in 0..num_workers {
            workers.push(spawn_worker(idx));
        }
        Self {
            workers,
            next: AtomicUsize::new(0),
        }
    }

    /// Submit a task to the next worker in round-robin order. Fails only if
    /// every worker thread has panicked and its receiver has been dropped.
    pub fn submit(&self, task: ScriptTask) -> Result<(), ScriptTask> {
        let len = self.workers.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed) % len;
        // Try workers starting at the round-robin index; fall through to the
        // next one only if a worker has crashed (sender closed).
        let mut task = Some(task);
        for offset in 0..len {
            let idx = (start + offset) % len;
            let t = task.take().expect("task slot must be populated");
            match self.workers[idx].send(t) {
                Ok(()) => return Ok(()),
                Err(err) => task = Some(err.0),
            }
        }
        Err(task.expect("task slot must be populated on failure path"))
    }
}

fn spawn_worker(index: usize) -> mpsc::UnboundedSender<ScriptTask> {
    let (tx, mut rx) = mpsc::unbounded_channel::<ScriptTask>();
    let name = format!("js-runtime-worker-{index}");
    std::thread::Builder::new()
        .name(name)
        .spawn(move || {
            let rt = Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build JS runtime worker tokio runtime");
            let local = LocalSet::new();
            local.spawn_local(async move {
                while let Some(task) = rx.recv().await {
                    // Spawn each task onto the LocalSet so independent scripts
                    // make progress concurrently on this single thread. A task
                    // that awaits (DB round-trip, `kv.watch` long-poll, timer)
                    // yields the thread to its peers instead of blocking them.
                    let future = (task.build)();
                    tokio::task::spawn_local(future);
                }
            });
            rt.block_on(local);
        })
        .expect("Failed to spawn JS runtime worker thread");
    tx
}

/// Process-wide worker pool shared by every `JsRuntime::execute_script` call.
static POOL: OnceLock<WorkerPool> = OnceLock::new();

/// Worker count for the global pool. Overridable via `SECUTILS_JS_WORKERS` for
/// local experimentation and CI; defaults to the parallelism reported by the
/// OS, with a floor of 2 so even tiny CI boxes keep some concurrency.
fn default_worker_count() -> usize {
    if let Ok(raw) = std::env::var("SECUTILS_JS_WORKERS")
        && let Ok(parsed) = raw.parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }

    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2)
}

/// Eagerly initialise the pool (called once from `JsRuntime::init_platform`).
/// Safe to call multiple times: subsequent calls are no-ops.
pub fn init() {
    POOL.get_or_init(|| WorkerPool::new(default_worker_count()));
}

/// Return the shared pool, lazily initialising it with `default_worker_count`
/// workers if `init` has not been called yet.
pub fn global() -> &'static WorkerPool {
    POOL.get_or_init(|| WorkerPool::new(default_worker_count()))
}
