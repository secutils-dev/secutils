use crate::config::JsRuntimeConfig;
use anyhow::{bail, Context};
use deno_core::{serde_v8, v8, PollEventLoopOptions, RuntimeOptions};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

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
    ) -> Result<R, anyhow::Error> {
        let now = Instant::now();

        let isolate_handle = self.inner_runtime.v8_isolate().thread_safe_handle();
        self.inner_runtime
            .add_near_heap_limit_callback(move |current_value, _| {
                log::error!(
                    "Approaching the memory limit of ({current_value}), terminating execution."
                );
                isolate_handle.terminate_execution();
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

        // Retrieve the result `Promise`.
        let promise = self
            .inner_runtime
            .execute_script("<anon>", js_code.into().into())?;

        // Wait for the promise to resolve.
        let resolve = self.inner_runtime.resolve(promise);
        let out = tokio::time::timeout(
            self.max_user_script_execution_time,
            self.inner_runtime
                .with_event_loop_promise(resolve, PollEventLoopOptions::default()),
        )
        .await??;

        let scope = &mut self.inner_runtime.handle_scope();
        let local = v8::Local::new(scope, out);
        let result =
            serde_v8::from_v8(scope, local).with_context(|| "Error deserializing script result");

        let execution_time = now.elapsed();
        log::info!(execution_time = execution_time.as_nanos(); "Executed user script in {:.2?}.", execution_time);

        result
    }
}
#[cfg(test)]
pub mod tests {
    use super::JsRuntime;
    use crate::JsRuntimeConfig;
    use deno_core::error::JsError;
    use serde::{Deserialize, Serialize};

    #[tokio::test]
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
        let result = runtime
            .execute_script::<ScriptContext>(
                r#"(async () => {{ return context; }})();"#,
                Some(script_context.clone()),
            )
            .await?;
        assert_eq!(result, script_context);

        // Can do basic math.
        let result = runtime
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

        // Limit execution time.
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
            .unwrap_err()
            .downcast::<tokio::time::error::Elapsed>()?;
        assert_eq!(format!("{result}"), "deadline has elapsed".to_string());

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
            .unwrap_err()
            .downcast::<JsError>()?;
        assert_eq!(
            result.exception_message,
            "Uncaught Error: execution terminated"
        );

        Ok(())
    }
}
