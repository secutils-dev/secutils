//! An elastic pool of worker threads, each owning a persistent `CurrentThread` tokio runtime. A
//! submitted script runs on exactly one worker thread and is driven to completion there
//! (`block_on`) before that worker accepts the next task - i.e. **at most one V8 isolate is ever
//! live per worker thread at a time**.
//!
//! ## Why one isolate per thread
//!
//! A V8 isolate is pinned to the thread that created it, and V8 tracks the "current isolate" and
//! the active `HandleScope` in *thread-local* state. Interleaving the event loops of two isolates
//! on a single OS thread (e.g. by `spawn_local`-ing several script futures onto one `LocalSet`)
//! corrupts that thread-local state: when one isolate parks on an `await` and another isolate
//! resumes on the same thread, V8 can try to create a handle while the thread's current
//! `HandleScope` belongs to the *other* isolate, aborting the process with a fatal
//! `v8::HandleScope::CreateHandle` ("Cannot create a handle without a HandleScope"). Debug V8
//! asserts this eagerly, release builds elide the check but the underlying state corruption is
//! still UB.
//!
//! So concurrency is achieved by running each concurrent script on its own thread, never by sharing
//! a thread between two isolates.
//!
//! ## Elasticity
//!
//! A baseline of `min_workers` threads is pre-spawned and kept warm so the common case pays no
//! thread-creation cost. When every worker is busy (for instance because several scripts are parked
//! on a `secutils.kv.watch` long-poll that can idle for tens of seconds), additional workers are
//! spawned on demand up to `max_workers`. Overflow workers above the baseline exit after
//! `IDLE_TIMEOUT` of inactivity so a burst of long-polls does not leave threads lingering forever.
//!
//! Back-pressure for user-visible workloads is already applied upstream (e.g. the
//! `max_concurrent_responder_requests` semaphore in the responder handler), so the task queue is
//! unbounded: queueing a cheap `ScriptTask` is preferable to blocking the producing future.

use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Condvar, Mutex, OnceLock},
    time::Duration,
};
use tokio::runtime::Builder;

/// How long an overflow worker (one spawned above `min_workers`) waits for new work before exiting
/// and releasing its thread + tokio runtime.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// A boxed, thread-movable closure that, when invoked on a worker thread, yields a (!Send) future
/// performing the actual script work. The future is `!Send` because V8 isolates are tied to the
/// thread that created them, it is block_on-ed to completion on the worker that picked up the task.
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

/// Mutable pool bookkeeping, guarded by [`Shared::lock`].
struct State {
    /// FIFO queue of tasks waiting for a free worker.
    tasks: VecDeque<ScriptTask>,
    /// Total number of live worker threads (busy + idle).
    total: usize,
    /// Number of worker threads currently parked waiting for a task.
    idle: usize,
}

struct Shared {
    lock: Mutex<State>,
    /// Signalled when a task is enqueued (wakes one parked worker).
    available: Condvar,
}

/// An elastic round-robin-free pool: any idle worker pops the next queued task, so work is
/// naturally balanced across whatever workers are free.
pub struct WorkerPool {
    shared: &'static Shared,
    min_workers: usize,
    max_workers: usize,
}

impl WorkerPool {
    fn new(min_workers: usize, max_workers: usize) -> Self {
        let min_workers = min_workers.max(1);
        let max_workers = max_workers.max(min_workers);
        let shared: &'static Shared = Box::leak(Box::new(Shared {
            lock: Mutex::new(State {
                tasks: VecDeque::new(),
                total: 0,
                idle: 0,
            }),
            available: Condvar::new(),
        }));

        let pool = Self {
            shared,
            min_workers,
            max_workers,
        };

        // Pre-spawn the warm baseline so the common path pays no thread-creation latency.
        {
            let mut state = shared.lock.lock().expect("worker pool mutex poisoned");
            for _ in 0..min_workers {
                state.total += 1;
                spawn_worker(shared, min_workers);
            }
        }

        pool
    }

    /// Submit a task. The task is enqueued and either handed to an already-idle worker or, if every
    /// worker is busy and the pool has not hit its ceiling, picked up by a freshly spawned worker.
    /// Returns `Err(task)` only if the pool somehow has no workers and cannot spawn one (it always
    /// can in practice), preserving the previous fallible signature for callers.
    pub fn submit(&self, task: ScriptTask) -> Result<(), ScriptTask> {
        let mut state = self.shared.lock.lock().expect("worker pool mutex poisoned");
        state.tasks.push_back(task);

        // Grow the pool when there is no idle worker ready to take the task and we still have
        // headroom. Otherwise wake a parked worker.
        if state.idle == 0 && state.total < self.max_workers {
            state.total += 1;
            spawn_worker(self.shared, self.min_workers);
        } else {
            self.shared.available.notify_one();
        }

        Ok(())
    }
}

fn spawn_worker(shared: &'static Shared, min_workers: usize) {
    std::thread::Builder::new()
        .name("js-runtime-worker".to_string())
        .spawn(move || worker_loop(shared, min_workers))
        .expect("Failed to spawn JS runtime worker thread");
}

fn worker_loop(shared: &'static Shared, min_workers: usize) {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build JS runtime worker tokio runtime");

    loop {
        // Acquire the next task, parking while the queue is empty. Overflow workers (those above
        // the warm baseline) exit after an idle period.
        let task = {
            let mut state = shared.lock.lock().expect("worker pool mutex poisoned");
            loop {
                if let Some(task) = state.tasks.pop_front() {
                    break Some(task);
                }

                state.idle += 1;
                let (guard, timed_out) = {
                    let (guard, wait_result) = shared
                        .available
                        .wait_timeout(state, IDLE_TIMEOUT)
                        .expect("worker pool mutex poisoned");
                    (guard, wait_result.timed_out())
                };
                state = guard;
                state.idle -= 1;

                // If a task showed up while we were waking, loop and take it.
                if !state.tasks.is_empty() {
                    continue;
                }

                // No work, and we timed out: retire this worker, but always keep the warm baseline
                // alive.
                if timed_out && state.total > min_workers {
                    state.total -= 1;
                    break None;
                }
            }
        };

        match task {
            // `block_on` drives the script to completion on this thread, no other isolate can run
            // here until it returns.
            Some(task) => rt.block_on((task.build)()),
            None => return,
        }
    }
}

/// Process-wide worker pool shared by every `JsRuntime::execute_script` call.
static POOL: OnceLock<WorkerPool> = OnceLock::new();

/// Warm baseline worker count. Overridable via `SECUTILS_JS_WORKERS` for local experimentation and
/// CI, defaults to the parallelism reported by the OS, with a floor of 2 so even tiny CI boxes keep
/// some concurrency.
fn min_worker_count() -> usize {
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

/// Hard ceiling on worker threads. The pool grows past the warm baseline to absorb bursts of
/// long-parked scripts (`kv.watch` long-polls), but never beyond this. Overridable vi
/// `SECUTILS_JS_MAX_WORKERS` environment variable.
fn max_worker_count(min: usize) -> usize {
    if let Ok(raw) = std::env::var("SECUTILS_JS_MAX_WORKERS")
        && let Ok(parsed) = raw.parse::<usize>()
        && parsed > 0
    {
        return parsed.max(min);
    }

    // Generous headroom for concurrent long-polls without risking unbounded thread growth, upstream
    // semaphores bound real concurrency well below this.
    min.max(512)
}

fn build_pool() -> WorkerPool {
    let min = min_worker_count();
    WorkerPool::new(min, max_worker_count(min))
}

/// Eagerly initialise the pool (called once from `JsRuntime::init_platform`).
/// Safe to call multiple times: subsequent calls are no-ops.
pub fn init() {
    POOL.get_or_init(build_pool);
}

/// Return the shared pool, lazily initialising it if `init` has not been called.
pub fn global() -> &'static WorkerPool {
    POOL.get_or_init(build_pool)
}
