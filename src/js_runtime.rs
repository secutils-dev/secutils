mod js_runtime_config;
mod op_proxy_request;
mod script_termination_reason;
mod worker_pool;

pub use self::{
    js_runtime_config::JsRuntimeConfig,
    op_proxy_request::{ProxyState, PublicUrlValidator},
};
use crate::js_runtime::{
    script_termination_reason::ScriptTerminationReason, worker_pool::ScriptTask,
};
use anyhow::Context;
use deno_core::{JsRuntimeForSnapshot, PollEventLoopOptions, RuntimeOptions, scope, serde_v8, v8};
use serde::Deserialize;
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tracing::error;

/// Defines a maximum interval on which script is checked for timeout.
const SCRIPT_TIMEOUT_CHECK_INTERVAL: Duration = Duration::from_secs(2);

deno_core::extension!(secutils_ext, ops = [op_proxy_request::op_proxy_request]);

/// Cached V8 startup snapshot, built once on the main thread at `init_platform`
/// time and reused by every subsequent `deno_core::JsRuntime::new` call across
/// every worker. `Box::leak` gives us the `&'static [u8]` shape required by
/// `RuntimeOptions::startup_snapshot`; the bytes live for the process lifetime,
/// which is exactly what we want.
static STARTUP_SNAPSHOT: OnceLock<&'static [u8]> = OnceLock::new();

fn build_startup_snapshot() -> &'static [u8] {
    // The snapshot intentionally does not bake in `secutils_ext` or any JS
    // modules: baking ops into a snapshot requires the snapshotting runtime
    // to register the exact same op layout at runtime (a minefield of subtle
    // version/feature mismatches). What we capture here is the expensive
    // part - the V8 context setup and builtin JS globals. `secutils_ext` is
    // still registered at `JsRuntime::new` time per invocation, but on top
    // of a warm, pre-initialised context.
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions::default());
    let snapshot = runtime.snapshot();
    Box::leak(snapshot) as &[u8]
}

fn startup_snapshot() -> &'static [u8] {
    STARTUP_SNAPSHOT.get_or_init(build_startup_snapshot)
}

/// Wraps the raw user script in a minimal async IIFE so we can use
/// `runtime.execute_script` + `runtime.resolve` uniformly whether the user
/// returned a promise or a value.
fn wrap_user_script_in_async_iife(script: &str) -> String {
    let script = script.trim().trim_end_matches(';');
    format!("(async () => (await ({script})))();")
}

/// Mutates `result` in place so that, if it is an object with a `body`
/// property, the body value becomes a `Uint8Array` of bytes suitable for
/// serde_v8 to deserialise into `Vec<u8>`.
///
/// The conversion rules match what the legacy JS wrapper did:
/// - `Uint8Array` / `ArrayBuffer` / typed-array views: passed through.
/// - `string`: UTF-8 encoded.
/// - numeric arrays (or empty arrays): copied into a `Uint8Array`.
/// - non-numeric arrays and any other objects/primitives: `JSON.stringify`
///   followed by UTF-8 encoding.
///
/// `null`/`undefined` body values are left untouched so `Option<Vec<u8>>`
/// fields can deserialise as `None`.
fn normalize_response_body_in_place(
    scope: &mut v8::PinScope<'_, '_>,
    result: v8::Local<v8::Value>,
) {
    let Ok(obj) = v8::Local::<v8::Object>::try_from(result) else {
        return;
    };
    let Some(body_key) = v8::String::new(scope, "body") else {
        return;
    };
    let body_key_v: v8::Local<v8::Value> = body_key.into();

    let Some(body) = obj.get(scope, body_key_v) else {
        return;
    };
    if body.is_null_or_undefined() {
        return;
    }
    if body.is_uint8_array() || body.is_array_buffer() || body.is_array_buffer_view() {
        return;
    }

    let replacement_bytes: Vec<u8> = if body.is_string() {
        v8::Local::<v8::String>::try_from(body)
            .map(|s| s.to_rust_string_lossy(scope).into_bytes())
            .unwrap_or_default()
    } else if body.is_array() {
        let Ok(arr) = v8::Local::<v8::Array>::try_from(body) else {
            return;
        };
        let len = arr.length();
        if len == 0 {
            Vec::new()
        } else {
            // The legacy JS picked the code path from the first element's type:
            // numeric first element -> treat every element as a u8 byte;
            // anything else -> JSON.stringify the whole array. Preserve that
            // exact semantic so behaviour is byte-identical to the old IIFE.
            let first = arr
                .get_index(scope, 0)
                .unwrap_or_else(|| v8::undefined(scope).into());
            if first.is_number() {
                let mut bytes = Vec::with_capacity(len as usize);
                for i in 0..len {
                    let v = arr
                        .get_index(scope, i)
                        .unwrap_or_else(|| v8::undefined(scope).into());
                    let n = v.number_value(scope).unwrap_or(0.0);
                    bytes.push(n as u8);
                }
                bytes
            } else {
                match v8::json::stringify(scope, body) {
                    Some(s) => s.to_rust_string_lossy(scope).into_bytes(),
                    None => return,
                }
            }
        }
    } else {
        match v8::json::stringify(scope, body) {
            Some(s) => s.to_rust_string_lossy(scope).into_bytes(),
            None => return,
        }
    };

    let len = replacement_bytes.len();
    let backing = v8::ArrayBuffer::new_backing_store_from_vec(replacement_bytes).make_shared();
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &backing);
    let Some(typed) = v8::Uint8Array::new(scope, buffer, 0, len) else {
        return;
    };
    obj.set(scope, body_key_v, typed.into());
}

/// An abstraction over the V8/Deno runtime that allows any utilities to execute custom user
/// JavaScript scripts. Script executions are dispatched to a process-wide pool of long-lived
/// worker threads (see [`worker_pool`]), each owning its own persistent `CurrentThread` tokio
/// runtime and `LocalSet`. A fresh V8 isolate is still created for every execution to preserve
/// isolation between scripts; what we avoid is rebuilding the surrounding tokio machinery on
/// every call.
pub struct JsRuntime;

impl JsRuntime {
    /// Initializes the JS runtime platform, builds the shared V8 startup
    /// snapshot, and eagerly spins up the worker pool. Should be called exactly
    /// once, from the main thread, during server startup.
    pub fn init_platform() {
        deno_core::JsRuntime::init_platform(None);
        // Build the snapshot on the main thread before any worker boots so the
        // first script execution on each worker does not pay for it. V8 requires
        // the snapshotting isolate to run on a single thread, which is why we
        // do it here rather than lazily inside a worker.
        let _ = startup_snapshot();
        worker_pool::init();
    }

    /// Executes a user script and returns the deserialised result.
    ///
    /// The script runs on one of the process-wide worker threads (round-robin scheduled),
    /// using that worker's long-lived tokio runtime and a fresh V8 isolate. This provides
    /// full isolation between scripts without paying the per-call cost of building a new
    /// tokio runtime every time.
    ///
    /// The raw user script is wrapped in a trivial async IIFE so callers can supply either
    /// a sync expression or a promise. After the script resolves, the returned object's
    /// `body` field (if present) is normalised in place to a `Uint8Array` - see
    /// [`normalize_response_body_in_place`] for the exact conversion rules. This keeps the
    /// responder code path (`Vec<u8>` bodies deserialised via `serde_bytes`) fast and
    /// avoids an async/await round-trip + JS conditional for every invocation.
    ///
    /// `js_script_context` is an optional JSON string that will be parsed by V8's native
    /// JSON parser and made available as the global `context` variable.
    pub async fn execute_script<R: for<'de> Deserialize<'de> + Send + 'static>(
        config: JsRuntimeConfig,
        js_code: String,
        js_script_context: Option<String>,
        proxy_state: Option<ProxyState>,
    ) -> Result<(R, Duration), anyhow::Error> {
        let wrapped = wrap_user_script_in_async_iife(&js_code);
        let (tx, rx) = oneshot::channel::<Result<(R, Duration), anyhow::Error>>();
        let task = ScriptTask::new(move || {
            Box::pin(async move {
                let result = Self::execute_script_internal::<R>(
                    config,
                    wrapped,
                    js_script_context,
                    proxy_state,
                )
                .await;
                // Ignore a dropped receiver: the caller awaiting `rx` either
                // got the result or gave up; either way, nothing to do.
                let _ = tx.send(result);
            })
        });

        worker_pool::global()
            .submit(task)
            .map_err(|_| anyhow::anyhow!("JS runtime worker pool unavailable"))?;

        rx.await
            .map_err(|_| anyhow::anyhow!("JS runtime worker dropped the script task"))?
    }

    async fn execute_script_internal<R: for<'de> Deserialize<'de>>(
        config: JsRuntimeConfig,
        js_code: String,
        js_script_context: Option<String>,
        proxy_state: Option<ProxyState>,
    ) -> Result<(R, Duration), anyhow::Error> {
        let now = Instant::now();

        let mut runtime = deno_core::JsRuntime::new(RuntimeOptions {
            create_params: Some(v8::Isolate::create_params().heap_limits(0, config.max_heap_size)),
            extensions: vec![secutils_ext::init()],
            startup_snapshot: Some(startup_snapshot()),
            ..Default::default()
        });

        if let Some(proxy_state) = proxy_state {
            let op_state = runtime.op_state();
            op_state.borrow_mut().put(proxy_state);
        }

        let termination_reason =
            Arc::new(AtomicUsize::new(ScriptTerminationReason::Unknown as usize));
        let timeout_token = Arc::new(AtomicBool::new(false));
        let isolate_handle = runtime.v8_isolate().thread_safe_handle();

        // Track memory usage and terminate execution if threshold is exceeded.
        let isolate_handle_clone = isolate_handle.clone();
        let termination_reason_clone = termination_reason.clone();
        let timeout_token_clone = timeout_token.clone();
        runtime.add_near_heap_limit_callback(move |current_value, _| {
            error!("Approaching the memory limit of ({current_value}), terminating execution.");

            isolate_handle_clone.terminate_execution();

            timeout_token_clone.swap(true, Ordering::Relaxed);
            termination_reason_clone.store(
                ScriptTerminationReason::MemoryLimit as usize,
                Ordering::Relaxed,
            );

            // Give the runtime enough heap to terminate without crashing the process.
            5 * current_value
        });

        // Set script context on a global scope if provided (parse JSON via V8's native parser
        // to avoid serde_json arbitrary_precision interop issues with serde_v8).
        if let Some(ref json_str) = js_script_context {
            scope!(scope, runtime);

            let context = scope.get_current_context();
            let scope = &mut v8::ContextScope::new(scope, context);

            let Some(context_key) = v8::String::new(scope, "context") else {
                anyhow::bail!("Cannot create script context key.");
            };
            let Some(json_v8) = v8::String::new(scope, json_str) else {
                anyhow::bail!("Cannot create V8 string for script context.");
            };
            let Some(context_value) = v8::json::parse(scope, json_v8) else {
                anyhow::bail!("Cannot parse script context JSON.");
            };
            context
                .global(scope)
                .set(scope, context_key.into(), context_value);
        }

        // Track the time the script takes to execute, and terminate execution if threshold is exceeded.
        let termination_timeout = config.max_user_script_execution_time;
        let termination_reason_clone = termination_reason.clone();
        let timeout_token_clone = timeout_token.clone();
        std::thread::spawn(move || {
            let now = Instant::now();
            loop {
                // If task is cancelled, return immediately.
                if timeout_token_clone.load(Ordering::Relaxed) {
                    return;
                }

                // Otherwise, terminate execution if time is out, or sleep for max `SCRIPT_TIMEOUT_CHECK_INTERVAL`.
                let Some(time_left) = termination_timeout.checked_sub(now.elapsed()) else {
                    termination_reason_clone.store(
                        ScriptTerminationReason::TimeLimit as usize,
                        Ordering::Relaxed,
                    );
                    isolate_handle.terminate_execution();
                    return;
                };

                std::thread::sleep(std::cmp::min(time_left, SCRIPT_TIMEOUT_CHECK_INTERVAL));
            }
        });

        let handle_error = |err: anyhow::Error| match ScriptTerminationReason::from(
            termination_reason.load(Ordering::Relaxed),
        ) {
            ScriptTerminationReason::MemoryLimit => err.context("Script exceeded memory limit."),
            ScriptTerminationReason::TimeLimit => err.context("Script exceeded time limit."),
            ScriptTerminationReason::Unknown => err,
        };

        let script_result_promise = runtime.execute_script("<anon>", js_code).map_err(|err| {
            timeout_token.swap(true, Ordering::Relaxed);
            runtime.v8_isolate().cancel_terminate_execution();
            handle_error(err.into())
        })?;

        let resolve = runtime.resolve(script_result_promise);
        // Wrap the event loop in a tokio timeout to handle async operations
        // (e.g. timers) that keep the event loop idle beyond the time limit.
        // The watchdog thread above handles synchronous loops that block V8.
        let script_result = match tokio::time::timeout(
            config.max_user_script_execution_time,
            runtime.with_event_loop_promise(resolve, PollEventLoopOptions::default()),
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(err)) => {
                timeout_token.swap(true, Ordering::Relaxed);
                runtime.v8_isolate().cancel_terminate_execution();
                return Err(handle_error(err.into()));
            }
            Err(_elapsed) => {
                timeout_token.swap(true, Ordering::Relaxed);
                runtime.v8_isolate().cancel_terminate_execution();
                return Err(anyhow::anyhow!("Script execution timed out")
                    .context("Script exceeded time limit."));
            }
        };

        // Abort termination thread, if script managed to complete.
        timeout_token.swap(true, Ordering::Relaxed);

        scope!(scope, runtime);

        let local = v8::Local::new(scope, script_result);
        normalize_response_body_in_place(scope, local);
        serde_v8::from_v8(scope, local)
            .map(|result| (result, now.elapsed()))
            .with_context(|| "Error deserializing script result")
    }
}

#[cfg(test)]
pub mod tests {
    use super::{JsRuntime, JsRuntimeConfig, ProxyState, PublicUrlValidator};
    use deno_core::error::{CoreError, CoreErrorKind};
    use futures::future::BoxFuture;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use url::Url;

    /// A mock URL validator that always approves URLs.
    #[derive(Clone)]
    struct AllowAllValidator;
    impl PublicUrlValidator for AllowAllValidator {
        fn is_public_web_url<'a>(&'a self, _url: &'a Url) -> BoxFuture<'a, bool> {
            Box::pin(futures::future::ready(true))
        }
    }

    /// A mock URL validator that always rejects URLs.
    #[derive(Clone)]
    struct DenyAllValidator;
    impl PublicUrlValidator for DenyAllValidator {
        fn is_public_web_url<'a>(&'a self, _url: &'a Url) -> BoxFuture<'a, bool> {
            Box::pin(futures::future::ready(false))
        }
    }

    fn test_proxy_state(restrict_to_public_urls: bool) -> ProxyState {
        ProxyState::new(
            Arc::new(AllowAllValidator),
            restrict_to_public_urls,
            10_485_760,
            std::time::Duration::from_secs(30),
        )
    }

    fn test_proxy_state_with_validator(
        validator: Arc<dyn PublicUrlValidator>,
        restrict: bool,
    ) -> ProxyState {
        ProxyState::new(
            validator,
            restrict,
            10_485_760,
            std::time::Duration::from_secs(30),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_scripts() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(5),
        };

        #[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
        struct ScriptContext {
            arg_num: usize,
            arg_str: String,
            arg_array: Vec<String>,
            arg_buf: Vec<u8>,
        }
        let script_context = ScriptContext {
            arg_num: 115,
            arg_str: "Hello, world!".to_string(),
            arg_array: vec!["one".to_string(), "two".to_string()],
            arg_buf: vec![1, 2, 3],
        };

        let (result, _) = JsRuntime::execute_script::<ScriptContext>(
            config,
            r#"(async () => {{ return context; }})();"#.to_string(),
            Some(serde_json::to_string(&script_context)?),
            None,
        )
        .await?;
        assert_eq!(result, script_context);

        let (result, _) = JsRuntime::execute_script::<usize>(
            config,
            r#"(async () => {{ return context.arg_num * 2; }})();"#.to_string(),
            Some(serde_json::to_string(&script_context)?),
            None,
        )
        .await?;
        assert_eq!(result, 230);

        let result = JsRuntime::execute_script::<()>(
            config,
            r#"(async () => {{ throw new Error("Uh oh."); }})();"#.to_string(),
            None,
            None,
        )
        .await
        .unwrap_err()
        .downcast::<CoreError>()?;
        if let CoreErrorKind::Js(ref js_error) = *result.0 {
            assert_eq!(
                js_error.exception_message,
                "Uncaught (in promise) Error: Uh oh."
            );
        } else {
            panic!("Expected JsError, got {result:?}");
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_limit_execution_time() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(5),
        };

        // Async timer that exceeds the execution time limit should be killed
        // by the tokio::time::timeout wrapper around the event loop.
        let result = JsRuntime::execute_script::<String>(
            config,
            r#"
        (async () => {{
            return new Promise((resolve) => {
                Deno.core.createTimer(
                    () => resolve("Done"),
                    10 * 1000,
                    undefined,
                    false,
                    true,
                    false
                );
            });
        }})();
        "#
            .to_string(),
            None,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(
            format!("{result}"),
            "Script exceeded time limit.".to_string()
        );

        let result = JsRuntime::execute_script::<String>(
            config,
            r#"
        (() => {{
            while (true) {}
        }})();
        "#
            .to_string(),
            None,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(
            format!("{result}"),
            "Script exceeded time limit.".to_string()
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_limit_execution_memory() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(5),
        };

        let result = JsRuntime::execute_script::<String>(
            config,
            r#"
        (async () => {{
           let s = "";
           while(true) { s += "Hello"; }
           return "Done";
        }})();
        "#
            .to_string(),
            None,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(
            format!("{result}"),
            "Script exceeded memory limit.".to_string()
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_forwards_request() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        let mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/hello");
                then.status(200)
                    .header("x-test", "test-value")
                    .body("upstream-response");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct ProxyResult {
            status_code: u16,
            headers: std::collections::HashMap<String, String>,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let url = format!("{}/hello", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        let (result, _) = JsRuntime::execute_script::<ProxyResult>(
            config,
            script,
            None,
            Some(test_proxy_state(false)),
        )
        .await?;

        assert_eq!(result.status_code, 200);
        assert_eq!(result.headers.get("x-test").unwrap(), "test-value");
        assert_eq!(String::from_utf8(result.body)?, "upstream-response");
        mock.assert_async().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_forwards_with_method_headers_body() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        let mock = mock_server
            .mock_async(|when, then| {
                when.method("POST")
                    .path("/api")
                    .header("content-type", "application/json")
                    .body(r#"{"key":"value"}"#);
                then.status(201).body("created");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct ProxyResult {
            status_code: u16,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let url = format!("{}/api", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                const body = Deno.core.encode(JSON.stringify({{ key: "value" }}));
                return await Deno.core.ops.op_proxy_request({{
                    url: "{url}",
                    method: "POST",
                    headers: {{ "content-type": "application/json" }},
                    body: Array.from(body),
                }});
            }})()"#
        );

        let (result, _) = JsRuntime::execute_script::<ProxyResult>(
            config,
            script,
            None,
            Some(test_proxy_state(false)),
        )
        .await?;

        assert_eq!(result.status_code, 201);
        assert_eq!(String::from_utf8(result.body)?, "created");
        mock.assert_async().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_transform_response() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/data");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(r#"{"original":true}"#);
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct ProxyResult {
            status_code: u16,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        // Script returns `body` as a plain JS number array. The runtime's body
        // normalisation converts it to a `Uint8Array` before serde_v8 sees it,
        // so `ProxyResult::body` deserialises via `serde_bytes`.
        let url = format!("{}/data", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                const resp = await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
                const body = JSON.parse(Deno.core.decode(new Uint8Array(resp.body)));
                body.modified = true;
                return {{
                    statusCode: resp.statusCode,
                    headers: resp.headers,
                    body: Array.from(Deno.core.encode(JSON.stringify(body))),
                }};
            }})()"#
        );

        let (result, _) = JsRuntime::execute_script::<ProxyResult>(
            config,
            script,
            None,
            Some(test_proxy_state(false)),
        )
        .await?;

        assert_eq!(result.status_code, 200);
        let body: serde_json::Value = serde_json::from_slice(&result.body)?;
        assert_eq!(body["original"], true);
        assert_eq!(body["modified"], true);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_rejects_invalid_url() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                return await Deno.core.ops.op_proxy_request({ url: "not-a-url" });
            })()"#
                .to_string(),
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("Invalid URL"),
            "Expected 'Invalid URL' in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_rejects_invalid_method() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                return await Deno.core.ops.op_proxy_request({
                    url: "http://localhost:1234",
                    method: "INVALID METHOD WITH SPACES",
                });
            })()"#
                .to_string(),
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("Invalid HTTP method"),
            "Expected 'Invalid HTTP method' in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_rejects_invalid_header_name() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                return await Deno.core.ops.op_proxy_request({
                    url: "http://localhost:1234",
                    headers: { "invalid header\nname": "value" },
                });
            })()"#
                .to_string(),
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("Invalid header name"),
            "Expected 'Invalid header name' in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_ssrf_rejects_non_public_url() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let proxy = test_proxy_state_with_validator(Arc::new(DenyAllValidator), true);
        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                return await Deno.core.ops.op_proxy_request({
                    url: "http://internal-service:8080/secret",
                });
            })()"#
                .to_string(),
            None,
            Some(proxy),
        )
        .await
        .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("URL not allowed"),
            "Expected 'URL not allowed' in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_ssrf_allows_when_disabled() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/");
                then.status(200).body("ok");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        // DenyAllValidator would reject, but restrict_to_public_urls is false so it's skipped.
        let proxy = test_proxy_state_with_validator(Arc::new(DenyAllValidator), false);
        let url = mock_server.base_url();
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct R {
            status_code: u16,
        }

        let (result, _) = JsRuntime::execute_script::<R>(config, script, None, Some(proxy)).await?;
        assert_eq!(result.status_code, 200);

        Ok(())
    }

    // `proxy_op_ssrf_with_mock_network` pulls in `crate::network::*`, which is
    // only wired up by `src/main.rs`. Compiling this test against the lib
    // target would fail to resolve those imports, so the whole test is gated
    // behind the `bin-tests` Cargo feature (off by default, on for
    // `cargo test --bin secutils --features bin-tests`).
    #[cfg(feature = "bin-tests")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_ssrf_with_mock_network() -> anyhow::Result<()> {
        use crate::network::{Network, tests::MockResolver};
        use hickory_resolver::proto::rr::{Name, RData, Record, rdata::A};
        use lettre::transport::stub::AsyncStubTransport;
        use reqwest_middleware::ClientBuilder;
        use std::net::Ipv4Addr;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        // Domain resolves to a private IP (127.0.0.1).
        let local_network = Network::new(
            MockResolver::new_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]),
            AsyncStubTransport::new_ok(),
            ClientBuilder::new(reqwest::Client::new()).build(),
        );

        let proxy = ProxyState::new(
            Arc::new(local_network),
            true,
            10_485_760,
            std::time::Duration::from_secs(30),
        );

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                return await Deno.core.ops.op_proxy_request({
                    url: "http://evil.example.com/secret",
                });
            })()"#
                .to_string(),
            None,
            Some(proxy),
        )
        .await
        .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("URL not allowed (non-public address)"),
            "Expected SSRF rejection in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_rejects_oversized_response() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/big");
                then.status(200).body("x".repeat(1024));
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let mut proxy = test_proxy_state(false);
        proxy.max_response_size = 100; // Only allow 100 bytes

        let url = format!("{}/big", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        let result =
            JsRuntime::execute_script::<serde_json::Value>(config, script, None, Some(proxy))
                .await
                .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("Upstream response body too large"),
            "Expected 'too large' in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_connect_failure() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        // Connect to a port that's (almost certainly) not listening.
        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                return await Deno.core.ops.op_proxy_request({
                    url: "http://127.0.0.1:19876/nope",
                });
            })()"#
                .to_string(),
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        let err_msg = format!("{result}");
        assert!(
            err_msg.contains("Failed to connect to upstream")
                || err_msg.contains("Upstream request failed"),
            "Expected connection error in: {err_msg}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_upstream_5xx_forwarded() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/error");
                then.status(503).body("service unavailable");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct R {
            status_code: u16,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let url = format!("{}/error", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        let (result, _) =
            JsRuntime::execute_script::<R>(config, script, None, Some(test_proxy_state(false)))
                .await?;

        assert_eq!(result.status_code, 503);
        assert_eq!(String::from_utf8(result.body)?, "service unavailable");

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_does_not_follow_redirects() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        let mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/old");
                then.status(302)
                    .header("location", "http://example.com/new")
                    .body("redirecting");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct R {
            status_code: u16,
            headers: std::collections::HashMap<String, String>,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let url = format!("{}/old", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        let (result, _) =
            JsRuntime::execute_script::<R>(config, script, None, Some(test_proxy_state(false)))
                .await?;

        assert_eq!(result.status_code, 302);
        assert_eq!(
            result.headers.get("location").unwrap(),
            "http://example.com/new"
        );
        assert_eq!(String::from_utf8(result.body)?, "redirecting");
        mock.assert_async().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_does_not_follow_301_redirect() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        let redirect_mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/moved");
                then.status(301)
                    .header("location", "/new-location")
                    .body("moved permanently");
            })
            .await;
        // If the client followed the redirect, it would hit /new-location.
        let target_mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/new-location");
                then.status(200).body("should not reach here");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct R {
            status_code: u16,
            headers: std::collections::HashMap<String, String>,
        }

        let url = format!("{}/moved", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        let (result, _) =
            JsRuntime::execute_script::<R>(config, script, None, Some(test_proxy_state(false)))
                .await?;

        assert_eq!(result.status_code, 301);
        assert_eq!(result.headers.get("location").unwrap(), "/new-location");
        redirect_mock.assert_async().await;
        assert_eq!(target_mock.calls_async().await, 0);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn crash_prevention_memory_bomb_does_not_crash_server() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => {
                let s = "";
                while(true) { s += "AAAAAAAAAAAAAAAA"; }
            })()"#
                .to_string(),
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        assert!(
            format!("{result}").contains("Script exceeded memory limit"),
            "Expected memory limit error: {result}"
        );

        // Verify we can still execute scripts after the memory bomb.
        let (result, _) = JsRuntime::execute_script::<usize>(
            config,
            "(async () => { return 42; })()".to_string(),
            None,
            None,
        )
        .await?;
        assert_eq!(result, 42);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn crash_prevention_infinite_loop_does_not_hang() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(3),
        };

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            r#"(async () => { while(true) {} })()"#.to_string(),
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        assert!(
            format!("{result}").contains("Script exceeded time limit"),
            "Expected time limit error: {result}"
        );

        // Verify we can still execute scripts.
        let (result, _) = JsRuntime::execute_script::<usize>(
            config,
            "(async () => { return 7; })()".to_string(),
            None,
            None,
        )
        .await?;
        assert_eq!(result, 7);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn scripts_without_proxy_still_work() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(5),
        };

        // Script that doesn't use op_proxy_request at all (regression test).
        let (result, _) = JsRuntime::execute_script::<usize>(
            config,
            "(async () => { return 1 + 2 + 3; })()".to_string(),
            None,
            None,
        )
        .await?;
        assert_eq!(result, 6);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_script_executions_are_isolated() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        // Launch multiple scripts concurrently and verify they don't interfere.
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let script = format!("(async () => {{ return {i} * 10; }})()");
                tokio::spawn(async move {
                    JsRuntime::execute_script::<usize>(config, script, None, None).await
                })
            })
            .collect();

        let mut results = Vec::new();
        for handle in handles {
            let (result, _) = handle.await??;
            results.push(result);
        }
        results.sort();
        assert_eq!(results, vec![0, 10, 20, 30, 40]);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_insecure_flag_works_with_http() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        let mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/insecure-test");
                then.status(200).body("insecure-ok");
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct R {
            status_code: u16,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let url = format!("{}/insecure-test", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}", insecure: true }});
            }})()"#
        );

        let (result, _) =
            JsRuntime::execute_script::<R>(config, script, None, Some(test_proxy_state(false)))
                .await?;

        assert_eq!(result.status_code, 200);
        assert_eq!(String::from_utf8(result.body)?, "insecure-ok");
        mock.assert_async().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_timeout_causes_error() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/slow");
                then.status(200)
                    .body("slow-response")
                    .delay(std::time::Duration::from_secs(5));
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let url = format!("{}/slow", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}", timeout: 100 }});
            }})()"#
        );

        let result = JsRuntime::execute_script::<serde_json::Value>(
            config,
            script,
            None,
            Some(test_proxy_state(false)),
        )
        .await
        .unwrap_err();

        assert!(
            format!("{result}").contains("timed out"),
            "Expected timeout error, got: {result}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_timeout_clamped_to_server_max() -> anyhow::Result<()> {
        let mock_server = httpmock::MockServer::start_async().await;
        mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/clamped");
                then.status(200)
                    .body("clamped-response")
                    .delay(std::time::Duration::from_secs(3));
            })
            .await;

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };

        let mut proxy = test_proxy_state(false);
        proxy.max_request_timeout = std::time::Duration::from_millis(200);

        let url = format!("{}/clamped", mock_server.base_url());
        // Script asks for 60s but server max is 200ms, so it should time out.
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}", timeout: 60000 }});
            }})()"#
        );

        let result =
            JsRuntime::execute_script::<serde_json::Value>(config, script, None, Some(proxy))
                .await
                .unwrap_err();

        assert!(
            format!("{result}").contains("timed out"),
            "Expected timeout error due to clamping, got: {result}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_decompresses_gzip_response() -> anyhow::Result<()> {
        let original = b"Hello, compressed world!";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, original)?;
        let compressed = encoder.finish()?;

        let mock_server = httpmock::MockServer::start_async().await;
        let mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/compressed");
                then.status(200)
                    .header("content-encoding", "gzip")
                    .header("content-type", "text/plain")
                    .body(compressed);
            })
            .await;

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct ProxyResult {
            status_code: u16,
            headers: std::collections::HashMap<String, String>,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };
        let url = format!("{}/compressed", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}" }});
            }})()"#
        );

        let (result, _) = JsRuntime::execute_script::<ProxyResult>(
            config,
            script,
            None,
            Some(test_proxy_state(false)),
        )
        .await?;

        assert_eq!(result.status_code, 200);
        assert_eq!(result.body, original);
        assert!(!result.headers.contains_key("content-encoding"));
        assert_eq!(
            result.headers.get("x-original-content-encoding").unwrap(),
            "gzip"
        );
        assert_eq!(result.headers.get("content-type").unwrap(), "text/plain");
        mock.assert_async().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_skips_decompression_when_disabled() -> anyhow::Result<()> {
        let original = b"Hello, compressed world!";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, original)?;
        let compressed = encoder.finish()?;

        let mock_server = httpmock::MockServer::start_async().await;
        let mock = mock_server
            .mock_async(|when, then| {
                when.method("GET").path("/raw");
                then.status(200)
                    .header("content-encoding", "gzip")
                    .body(compressed.clone());
            })
            .await;

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct ProxyResult {
            status_code: u16,
            headers: std::collections::HashMap<String, String>,
            #[serde(with = "serde_bytes")]
            body: Vec<u8>,
        }

        let config = JsRuntimeConfig {
            max_heap_size: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(10),
        };
        let url = format!("{}/raw", mock_server.base_url());
        let script = format!(
            r#"(async () => {{
                return await Deno.core.ops.op_proxy_request({{ url: "{url}", decompress: false }});
            }})()"#
        );

        let (result, _) = JsRuntime::execute_script::<ProxyResult>(
            config,
            script,
            None,
            Some(test_proxy_state(false)),
        )
        .await?;

        assert_eq!(result.status_code, 200);
        assert_eq!(result.body, compressed);
        assert_eq!(result.headers.get("content-encoding").unwrap(), "gzip");
        assert!(!result.headers.contains_key("x-original-content-encoding"));
        mock.assert_async().await;

        Ok(())
    }

    // Reaches for `crate::utils::webhooks::ResponderScriptResult`, which is
    // only in scope from the main binary. Gated behind the `bin-tests` Cargo
    // feature so that `cargo clippy --all-targets` (which compiles the lib as
    // a test) doesn't hit unresolved imports.
    #[cfg(feature = "bin-tests")]
    mod body_auto_convert {
        use super::*;
        use crate::utils::webhooks::ResponderScriptResult;

        fn config() -> JsRuntimeConfig {
            JsRuntimeConfig {
                max_heap_size: 10 * 1024 * 1024,
                max_user_script_execution_time: std::time::Duration::from_secs(5),
            }
        }

        async fn run_script(user_script: &str) -> anyhow::Result<ResponderScriptResult> {
            let (result, _) = JsRuntime::execute_script::<ResponderScriptResult>(
                config(),
                user_script.to_string(),
                None,
                None,
            )
            .await?;
            Ok(result)
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_uint8array() -> anyhow::Result<()> {
            let result =
                run_script(r#"(async () => ({ body: new Uint8Array([72, 73]) }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(&[72u8, 73][..]));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_deno_core_encode() -> anyhow::Result<()> {
            let result =
                run_script(r#"(async () => ({ body: Deno.core.encode("AB") }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(&[65u8, 66][..]));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_string() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: "Hello" }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(b"Hello".as_slice()));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_object() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: { key: "value" } }))()"#).await?;
            assert_eq!(
                result.body.as_deref(),
                Some(br#"{"key":"value"}"#.as_slice())
            );
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_array_of_objects() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: [{ a: 1 }] }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(br#"[{"a":1}]"#.as_slice()));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_array_of_numbers() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: [65, 66, 67] }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(&[65u8, 66, 67][..]));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_empty_array() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: [] }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(&[][..]));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_number() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: 42 }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(b"42".as_slice()));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_boolean() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: true }))()"#).await?;
            assert_eq!(result.body.as_deref(), Some(b"true".as_slice()));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_null() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: null }))()"#).await?;
            assert_eq!(result.body, None);
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_undefined() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ statusCode: 204 }))()"#).await?;
            assert_eq!(result.body, None);
            assert_eq!(result.status_code, Some(204));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn body_array_from_encode() -> anyhow::Result<()> {
            let result =
                run_script(r#"(async () => ({ body: Array.from(Deno.core.encode("Hi")) }))()"#)
                    .await?;
            assert_eq!(result.body.as_deref(), Some(b"Hi".as_slice()));
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn script_with_trailing_semicolon() -> anyhow::Result<()> {
            let result = run_script(r#"(async () => ({ body: "ok" }))();"#).await?;
            assert_eq!(result.body.as_deref(), Some(b"ok".as_slice()));
            Ok(())
        }
    }
}
