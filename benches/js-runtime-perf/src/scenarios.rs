//! Scenario catalogue for the Secutils harness. Every scenario returns a
//! [`ScenarioResult`]; the driver collects them into a single JSON report.
//!
//! Scenarios intentionally use the real [`secutils::js_runtime::JsRuntime`]
//! entry points so any change to that module (e.g. introducing a worker pool,
//! sharing `reqwest::Client`s, or adding a V8 startup snapshot) is reflected
//! in the measured numbers.

mod cold_start;
mod common;
mod concurrent_responders;
mod proxy_request;
mod responder_like;
mod steady_state;

use crate::measure::ScenarioResult;

/// Canonical ordering for scenarios, mirrored in `.perf/config.json`.
pub const ALL: &[&str] = &[
    "cold_start_trivial",
    "steady_state_trivial",
    "responder_like",
    "proxy_request",
    "concurrent_responders_8x",
];

pub async fn run(
    name: &str,
    iterations: u64,
    warmup: u64,
    concurrency: u64,
) -> anyhow::Result<ScenarioResult> {
    match name {
        "cold_start_trivial" => cold_start::run(iterations, warmup).await,
        "steady_state_trivial" => steady_state::run(iterations, warmup).await,
        "responder_like" => responder_like::run(iterations, warmup).await,
        "proxy_request" => proxy_request::run(iterations, warmup).await,
        "concurrent_responders_8x" => {
            concurrent_responders::run(iterations, warmup, concurrency).await
        }
        other => anyhow::bail!("unknown scenario `{other}`"),
    }
}
