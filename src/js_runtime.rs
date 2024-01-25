mod script_termination_reason;

use crate::{
    config::JsRuntimeConfig, js_runtime::script_termination_reason::ScriptTerminationReason,
};
use anyhow::{bail, Context};
use deno_core::{serde_v8, v8, PollEventLoopOptions, RuntimeOptions};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

/// Defines a maximum interval on which script is checked for timeout.
const SCRIPT_TIMEOUT_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// An abstraction over the V8/Deno runtime that allows any utilities to execute custom user
/// JavaScript scripts.
pub struct JsRuntime {
    inner_runtime: deno_core::JsRuntime,
    max_user_script_execution_time: Duration,
}

impl JsRuntime {
    /// Creates a new instance of the runtime.
    pub fn new(config: &JsRuntimeConfig) -> Self {
        Self {
            inner_runtime: deno_core::JsRuntime::new(RuntimeOptions {
                create_params: Some(
                    v8::Isolate::create_params().heap_limits(0, config.max_heap_size_bytes),
                ),
                ..Default::default()
            }),
            max_user_script_execution_time: config.max_user_script_execution_time,
        }
    }

    /// Initializes the JS runtime platform, should be called only once and in the main thread.
    pub fn init_platform() {
        deno_core::JsRuntime::init_platform(None);
    }

    /// Executes a user script and returns the result.
    pub async fn execute_script<R: for<'de> Deserialize<'de>>(
        &mut self,
        js_code: impl Into<String>,
        js_script_context: Option<impl Serialize>,
    ) -> Result<(R, Duration), anyhow::Error> {
        let now = Instant::now();

        let termination_reason =
            Arc::new(AtomicUsize::new(ScriptTerminationReason::Unknown as usize));
        let timeout_token = Arc::new(AtomicBool::new(false));
        let isolate_handle = self.inner_runtime.v8_isolate().thread_safe_handle();

        // Track memory usage and terminate execution if threshold is exceeded.
        let isolate_handle_clone = isolate_handle.clone();
        let termination_reason_clone = termination_reason.clone();
        let timeout_token_clone = timeout_token.clone();
        self.inner_runtime
            .add_near_heap_limit_callback(move |current_value, _| {
                log::error!(
                    "Approaching the memory limit of ({current_value}), terminating execution."
                );

                // Define termination reason and terminate execution.
                isolate_handle_clone.terminate_execution();

                timeout_token_clone.swap(true, Ordering::Relaxed);
                termination_reason_clone.store(
                    ScriptTerminationReason::MemoryLimit as usize,
                    Ordering::Relaxed,
                );

                // Give the runtime enough heap to terminate without crashing the process.
                5 * current_value
            });

        // Set script context on a global scope if provided.
        if let Some(script_context) = js_script_context {
            let scope = &mut self.inner_runtime.handle_scope();
            let context = scope.get_current_context();
            let scope = &mut v8::ContextScope::new(scope, context);

            let Some(context_key) = v8::String::new(scope, "context") else {
                bail!("Cannot create script context key.");
            };
            let context_value = serde_v8::to_v8(scope, script_context)
                .with_context(|| "Cannot serialize script context")?;
            context
                .global(scope)
                .set(scope, context_key.into(), context_value);
        }

        // Track the time the script takes to execute, and terminate execution if threshold is exceeded.
        let termination_timeout = self.max_user_script_execution_time;
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

        // Retrieve the result `Promise`.
        let script_result_promise = self
            .inner_runtime
            .execute_script("<anon>", js_code.into().into())
            .map_err(|err| {
                timeout_token.swap(true, Ordering::Relaxed);
                self.inner_runtime.v8_isolate().cancel_terminate_execution();
                handle_error(err)
            })?;

        // Wait for the promise to resolve.
        let resolve = self.inner_runtime.resolve(script_result_promise);
        let script_result = self
            .inner_runtime
            .with_event_loop_promise(resolve, PollEventLoopOptions::default())
            .await
            .map_err(|err| {
                timeout_token.swap(true, Ordering::Relaxed);
                self.inner_runtime.v8_isolate().cancel_terminate_execution();
                handle_error(err)
            })?;

        // Abort termination thread, if script managed to complete.
        timeout_token.swap(true, Ordering::Relaxed);

        let scope = &mut self.inner_runtime.handle_scope();
        let local = v8::Local::new(scope, script_result);
        serde_v8::from_v8(scope, local)
            .map(|result| (result, now.elapsed()))
            .with_context(|| "Error deserializing script result")
    }
}
#[cfg(test)]
pub mod tests {
    use super::JsRuntime;
    use crate::JsRuntimeConfig;
    use deno_core::error::JsError;
    use serde::{Deserialize, Serialize};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_scripts() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size_bytes: 10 * 1024 * 1024,
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

        // Can access script context.
        let mut runtime = JsRuntime::new(&config);
        let (result, _) = runtime
            .execute_script::<ScriptContext>(
                r#"(async () => {{ return context; }})();"#,
                Some(script_context.clone()),
            )
            .await?;
        assert_eq!(result, script_context);

        // Can do basic math.
        let (result, _) = runtime
            .execute_script::<usize>(
                r#"(async () => {{ return context.arg_num * 2; }})();"#,
                Some(script_context.clone()),
            )
            .await?;
        assert_eq!(result, 230);

        // Returns error from scripts
        let result = runtime
            .execute_script::<()>(
                r#"(async () => {{ throw new Error("Uh oh."); }})();"#,
                None::<()>,
            )
            .await
            .unwrap_err()
            .downcast::<JsError>()?;
        assert_eq!(
            result.exception_message,
            "Uncaught (in promise) Error: Uh oh."
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_limit_execution_time() -> anyhow::Result<()> {
        let config = JsRuntimeConfig {
            max_heap_size_bytes: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(5),
        };

        let mut runtime = JsRuntime::new(&config);

        // Limit execution time (async).
        let result = runtime
            .execute_script::<String>(
                r#"
        (async () => {{
            return new Promise((resolve) => {
                Deno.core.queueTimer(
                    Deno.core.getTimerDepth() + 1,
                    false,
                    10 * 1000,
                    () => resolve("Done")
                );
            });
        }})();
        "#,
                None::<()>,
            )
            .await
            .unwrap_err();
        assert_eq!(
            format!("{result}"),
            "Script exceeded time limit.".to_string()
        );

        // Limit execution time (sync).
        let result = runtime
            .execute_script::<String>(
                r#"
        (() => {{
            while (true) {}
        }})();
        "#,
                None::<()>,
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
            max_heap_size_bytes: 10 * 1024 * 1024,
            max_user_script_execution_time: std::time::Duration::from_secs(5),
        };

        let mut runtime = JsRuntime::new(&config);

        // Limit memory usage.
        let result = runtime
            .execute_script::<String>(
                r#"
        (async () => {{
           let s = "";
           while(true) { s += "Hello"; }
           return "Done";
        }})();
        "#,
                None::<()>,
            )
            .await
            .unwrap_err();
        assert_eq!(
            format!("{result}"),
            "Script exceeded memory limit.".to_string()
        );

        Ok(())
    }
}
