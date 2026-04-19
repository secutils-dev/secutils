//! `cold_start_trivial`: one-shot execution of a trivial script. Each
//! iteration pays the full cost of the current Secutils design:
//!
//! - `tokio::task::spawn_blocking`
//! - a brand-new `tokio::runtime::Builder::new_current_thread()` per call
//! - a fresh V8 isolate (no snapshot)
//! - watchdog thread spawn + heap-limit callback registration
//!
//! Tier 1 #1 (worker pool) and Tier 1 #3 (V8 snapshot) should both land
//! improvements visible in this scenario's p50/p99.

use crate::{
    measure::{Recorder, ScenarioResult, now},
    scenarios::common::{TRIVIAL_JS, default_config},
};
use secutils::js_runtime::JsRuntime;

pub async fn run(iterations: u64, warmup: u64) -> anyhow::Result<ScenarioResult> {
    for _ in 0..warmup {
        let _ = execute_once().await?;
    }

    let mut recorder = Recorder::new(iterations, warmup)?;
    for _ in 0..iterations {
        let start = now();
        execute_once().await?;
        recorder.observe(start.elapsed())?;
    }

    Ok(recorder.finalise())
}

async fn execute_once() -> anyhow::Result<u64> {
    let (result, _) =
        JsRuntime::execute_script::<u64>(default_config(), TRIVIAL_JS.to_string(), None, None)
            .await?;
    Ok(result)
}
