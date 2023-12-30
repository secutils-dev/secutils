use std::time::Duration;

/// Configuration for the JS runtime (Deno).
#[derive(Clone, Debug)]
pub struct JsRuntimeConfig {
    /// The hard limit for the JS runtime heap size in bytes.
    pub max_heap_size_bytes: usize,
    /// The maximum duration for a single JS script execution.
    pub max_user_script_execution_time: Duration,
}
