//! `concurrent_responders_8x`: issues `concurrency` script executions through
//! `tokio::spawn` in parallel, approximating a burst of webhook traffic. The
//! recorded latency is the wall clock of each individual execution, which
//! shows whether a future worker pool actually parallelises requests rather
//! than just serialising them faster.

use crate::{
    measure::{Recorder, ScenarioResult, now},
    scenarios::common::{TRIVIAL_JS, default_config},
};
use anyhow::Context;
use futures::future::try_join_all;
use secutils::js_runtime::JsRuntime;
use std::time::Duration;

pub async fn run(iterations: u64, warmup: u64, concurrency: u64) -> anyhow::Result<ScenarioResult> {
    assert!(concurrency >= 1, "concurrency must be ≥ 1");

    for _ in 0..warmup {
        execute_batch(concurrency).await?;
    }

    // `iterations` is the number of individual executions we want to measure,
    // split into batches of `concurrency`. Round up so we never miss samples.
    let batches = iterations.div_ceil(concurrency);
    let total = batches * concurrency;
    let mut recorder = Recorder::new(total, warmup)?;

    for _ in 0..batches {
        let durations = execute_batch(concurrency).await?;
        for duration in durations {
            recorder.observe(duration)?;
        }
    }

    Ok(recorder.finalise())
}

async fn execute_batch(concurrency: u64) -> anyhow::Result<Vec<Duration>> {
    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            tokio::spawn(async {
                let start = now();
                JsRuntime::execute_script::<u64>(
                    default_config(),
                    TRIVIAL_JS.to_string(),
                    None,
                    None,
                )
                .await?;
                Ok::<_, anyhow::Error>(start.elapsed())
            })
        })
        .collect();

    let results = try_join_all(handles)
        .await
        .context("concurrent script task panicked")?;

    results.into_iter().collect()
}
