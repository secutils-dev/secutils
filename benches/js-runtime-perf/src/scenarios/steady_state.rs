//! `steady_state_trivial`: runs the trivial script serially so the per-call
//! overhead (not startup) dominates. Amortises any one-time costs that might
//! creep into `cold_start_trivial` and exposes the raw cost of creating a
//! fresh isolate + CurrentThread runtime per invocation.

use crate::{
    measure::{Recorder, ScenarioResult, now},
    scenarios::common::{TRIVIAL_JS, default_config},
};
use secutils::js_runtime::JsRuntime;

pub async fn run(iterations: u64, warmup: u64) -> anyhow::Result<ScenarioResult> {
    for _ in 0..warmup {
        execute_once().await?;
    }

    let mut recorder = Recorder::new(iterations, warmup)?;
    for _ in 0..iterations {
        let start = now();
        execute_once().await?;
        recorder.observe(start.elapsed())?;
    }

    Ok(recorder.finalise())
}

async fn execute_once() -> anyhow::Result<()> {
    JsRuntime::execute_script::<u64>(default_config(), TRIVIAL_JS.to_string(), None, None).await?;
    Ok(())
}
