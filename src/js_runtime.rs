mod js_runtime_config;
mod op_proxy_request;
mod script_termination_reason;

pub use self::{
    js_runtime_config::JsRuntimeConfig,
    op_proxy_request::{ProxyState, PublicUrlValidator},
};
use crate::js_runtime::script_termination_reason::ScriptTerminationReason;
use anyhow::Context;
use deno_core::{PollEventLoopOptions, RuntimeOptions, scope, serde_v8, v8};
use serde::Deserialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tracing::error;

/// Defines a maximum interval on which script is checked for timeout.
const SCRIPT_TIMEOUT_CHECK_INTERVAL: Duration = Duration::from_secs(2);

deno_core::extension!(secutils_ext, ops = [op_proxy_request::op_proxy_request]);

/// Wraps a user script in an async IIFE that auto-converts the `body` field
/// of the returned object: strings are UTF-8 encoded, objects/arrays are
/// JSON-serialized, and plain number arrays become `Uint8Array` for backward
/// compatibility.  `Uint8Array`/`ArrayBuffer` values pass through unchanged.
pub fn wrap_script_with_body_conversion(script: &str) -> String {
    let script = script.trim().trim_end_matches(';');
    format!(
        r#"(async (globalThis) => {{
  const __result = await ({script});
  if (__result && __result.body !== undefined && __result.body !== null) {{
    const __body = __result.body;
    if (__body instanceof Uint8Array || __body instanceof ArrayBuffer || ArrayBuffer.isView(__body)) {{
    }} else if (typeof __body === 'string') {{
      __result.body = Deno.core.encode(__body);
    }} else if (Array.isArray(__body)) {{
      if (__body.length === 0 || typeof __body[0] === 'number') {{
        __result.body = new Uint8Array(__body);
      }} else {{
        __result.body = Deno.core.encode(JSON.stringify(__body));
      }}
    }} else {{
      __result.body = Deno.core.encode(JSON.stringify(__body));
    }}
  }}
  return __result;
}})(globalThis);"#
    )
}

/// An abstraction over the V8/Deno runtime that allows any utilities to execute custom user
/// JavaScript scripts. Each invocation runs inside a dedicated `spawn_blocking` task with its own
/// `CurrentThread` tokio runtime so that async Deno ops (e.g. `op_proxy_request`) work correctly.
pub struct JsRuntime;

impl JsRuntime {
    /// Initializes the JS runtime platform, should be called only once and in the main thread.
    pub fn init_platform() {
        deno_core::JsRuntime::init_platform(None);
    }

    /// Executes a user script and returns the result. The script runs inside a `spawn_blocking`
    /// task with its own `CurrentThread` tokio runtime and V8 isolate, providing full isolation
    /// from other concurrent scripts and the main server.
    ///
    /// `js_script_context` is an optional JSON string that will be parsed by V8's native JSON
    /// parser and made available as the global `context` variable.
    pub async fn execute_script<R: for<'de> Deserialize<'de> + Send + 'static>(
        config: JsRuntimeConfig,
        js_code: String,
        js_script_context: Option<String>,
        proxy_state: Option<ProxyState>,
    ) -> Result<(R, Duration), anyhow::Error> {
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("Failed to build CurrentThread tokio runtime for script execution")?;
            rt.block_on(async {
                Self::execute_script_internal(config, js_code, js_script_context, proxy_state).await
            })
        })
        .await
        .map_err(|join_err| anyhow::anyhow!("Script execution task panicked: {join_err}"))?
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
        let script_result = runtime
            .with_event_loop_promise(resolve, PollEventLoopOptions::default())
            .await
            .map_err(|err| {
                timeout_token.swap(true, Ordering::Relaxed);
                runtime.v8_isolate().cancel_terminate_execution();
                handle_error(err.into())
            })?;

        // Abort termination thread, if script managed to complete.
        timeout_token.swap(true, Ordering::Relaxed);

        scope!(scope, runtime);

        let local = v8::Local::new(scope, script_result);
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
    use reqwest_middleware::ClientBuilder;
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

        let result = JsRuntime::execute_script::<String>(
            config,
            r#"
        (async () => {{
            return new Promise((resolve) => {
                Deno.core.queueUserTimer(
                    Deno.core.getTimerDepth() + 1,
                    false,
                    10 * 1000,
                    () => resolve("Done")
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
            body: Vec<u8>,
        }

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxy_op_ssrf_with_mock_network() -> anyhow::Result<()> {
        use crate::network::{Network, tests::MockResolver};
        use lettre::transport::stub::AsyncStubTransport;
        use std::net::Ipv4Addr;
        use trust_dns_resolver::{
            Name,
            proto::rr::{RData, Record, rdata::A},
        };

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

    mod body_auto_convert {
        use super::*;
        use crate::{
            js_runtime::wrap_script_with_body_conversion, utils::webhooks::ResponderScriptResult,
        };

        fn config() -> JsRuntimeConfig {
            JsRuntimeConfig {
                max_heap_size: 10 * 1024 * 1024,
                max_user_script_execution_time: std::time::Duration::from_secs(5),
            }
        }

        async fn run_script(user_script: &str) -> anyhow::Result<ResponderScriptResult> {
            let (result, _) = JsRuntime::execute_script::<ResponderScriptResult>(
                config(),
                wrap_script_with_body_conversion(user_script),
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
