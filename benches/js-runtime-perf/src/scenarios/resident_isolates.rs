//! `resident_isolates`: holds `concurrency` responder scripts in flight at the
//! same time (each parked on a timer) so every V8 isolate + worker thread +
//! per-worker tokio runtime is simultaneously resident, then reports the peak
//! RSS growth. Divide `peak_rss_delta_kb` by `concurrency` for the marginal
//! cost of one concurrently-resident script.
//!
//! This is an **on-demand** scenario (not part of the default `all` run): it is
//! meant for capacity planning, e.g.
//!   `js-runtime-perf --scenarios resident_isolates --concurrency 1000`
//! optionally with `RESIDENT_PARK_MS` to widen the park window for large N.
//!
//! The latency columns reflect each script's wall-clock (≈ the park duration)
//! and are not the point - read the `rss` column.

use crate::measure::{Recorder, ScenarioResult, now, rss_kb};
use anyhow::Context;
use secutils::js_runtime::{JsRuntime, JsRuntimeConfig};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::Duration,
};

/// Wall-clock ceiling for a parked script. Must comfortably exceed the park
/// window so a resident isolate is never force-terminated mid-measurement.
const MAX_PARK_MS: u64 = 55_000;

/// 10 MiB heap (production default) but a 60 s execution limit so a wide park
/// window survives without the watchdog killing the isolate.
fn config() -> JsRuntimeConfig {
    JsRuntimeConfig {
        max_heap_size: 10 * 1024 * 1024,
        max_user_script_execution_time: Duration::from_secs(60),
    }
}

/// A script that parks for `ms` milliseconds, keeping its isolate resident.
fn park_js(ms: u64) -> String {
    format!(
        "new Promise((resolve) => {{ Deno.core.createTimer(() => resolve(42), {ms}, undefined, false, true, false); }})"
    )
}

pub async fn run(
    _iterations: u64,
    warmup: u64,
    concurrency: u64,
) -> anyhow::Result<ScenarioResult> {
    assert!(concurrency >= 1, "concurrency must be ≥ 1");

    let park_ms: u64 = std::env::var("RESIDENT_PARK_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000)
        .clamp(500, MAX_PARK_MS);

    // Warm the snapshot build + baseline worker pool before sampling so the
    // delta reflects only the cost of the resident isolates, not one-time init.
    for _ in 0..warmup.max(1) {
        JsRuntime::execute_script::<u64>(config(), park_js(1), None, None)
            .await
            .context("warmup script failed")?;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;

    let baseline_kb = rss_kb();
    let peak_kb = Arc::new(AtomicI64::new(baseline_kb));
    let stop = Arc::new(AtomicBool::new(false));

    let sampler = {
        let peak_kb = peak_kb.clone();
        let stop = stop.clone();
        tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                peak_kb.fetch_max(rss_kb(), Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
    };

    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            tokio::spawn(async move {
                let start = now();
                JsRuntime::execute_script::<u64>(config(), park_js(park_ms), None, None)
                    .await
                    .map(|_| start.elapsed())
            })
        })
        .collect();

    let mut durations = Vec::with_capacity(concurrency as usize);
    for handle in handles {
        let duration = handle
            .await
            .context("resident task panicked")?
            .context("resident script failed")?;
        durations.push(duration);
    }

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.await;

    let peak_kb = peak_kb.load(Ordering::Relaxed);

    // Reuse the histogram math for the latency columns, then override the RSS
    // delta with the peak we sampled while every isolate was resident (the
    // Recorder only samples at `observe` time, i.e. after the park ended).
    let mut recorder = Recorder::new(concurrency, warmup)?;
    for duration in durations {
        recorder.observe(duration)?;
    }
    let mut result = recorder.finalise();
    result.peak_rss_delta_kb = (peak_kb - baseline_kb).max(0);
    Ok(result)
}
