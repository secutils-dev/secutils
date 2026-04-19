//! Secutils JS runtime performance harness.
//!
//! Runs a fixed catalogue of scenarios against the real [`secutils::js_runtime::JsRuntime`]
//! and writes a single JSON document describing per-scenario latency percentiles,
//! throughput, and peak RSS delta. The output is consumed by `scripts/analyze-perf.ts`,
//! which appends one line per run to `.perf/history.jsonl`.
//!
//! See `AGENTS.md` for the user-facing contract; this file is the driver.

mod measure;
mod report;
mod scenarios;

use anyhow::Context;
use clap::Parser;
use report::Report;
use std::{path::PathBuf, process::ExitCode};

/// CLI arguments for the perf driver.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "js-runtime-perf",
    about = "Measure Secutils JS runtime performance across a fixed scenario catalogue."
)]
struct Args {
    /// Comma-separated list of scenarios to run, or `all` for every scenario.
    #[arg(long, default_value = "all")]
    scenarios: String,

    /// Number of measured iterations per scenario (after warmup).
    #[arg(long, default_value_t = 500)]
    iterations: u64,

    /// Number of warmup iterations per scenario that are discarded.
    #[arg(long, default_value_t = 50)]
    warmup: u64,

    /// Number of concurrent tasks for the `concurrent_responders_8x` scenario.
    #[arg(long, default_value_t = 8)]
    concurrency: u64,

    /// Output file path for the JSON report.
    #[arg(long, default_value = "/tmp/perf.json")]
    output: PathBuf,
}

fn main() -> ExitCode {
    // Many scenarios build a fresh CurrentThread tokio runtime + V8 isolate per call,
    // which on macOS each consume a kqueue fd. The default soft `RLIMIT_NOFILE` is 256
    // on macOS, so a few hundred iterations can exhaust the budget before Drop catches
    // up. Raise the soft limit to the hard limit so the harness surfaces *runtime*
    // overhead, not OS fd ceilings.
    if let Err(err) = raise_fd_limit() {
        eprintln!("warning: failed to raise RLIMIT_NOFILE: {err}");
    }

    secutils::js_runtime::JsRuntime::init_platform();

    let args = Args::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(std::cmp::max(
            2,
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        ))
        .enable_all()
        .build()
        .context("Failed to build driver Tokio runtime")
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("{err:?}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("perf driver failed: {err:?}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> anyhow::Result<()> {
    let selected = parse_scenarios(&args.scenarios);
    let mut report = Report::new();

    for name in scenarios::ALL {
        if !selected.iter().any(|s| s == "all" || s == name) {
            continue;
        }

        eprintln!("▶ {name}");
        let result = scenarios::run(name, args.iterations, args.warmup, args.concurrency)
            .await
            .with_context(|| format!("scenario `{name}` failed"))?;
        eprintln!(
            "  p50={:>6}µs  p90={:>6}µs  p99={:>6}µs  max={:>7}µs  ops/s={:>8.1}  rss_delta_kb={:>6}",
            result.p50_us,
            result.p90_us,
            result.p99_us,
            result.max_us,
            result.throughput_ops_per_sec,
            result.peak_rss_delta_kb
        );
        report.add(name, result);
    }

    report.write(&args.output).context("writing JSON report")?;
    eprintln!("✓ wrote report to {}", args.output.display());
    Ok(())
}

fn parse_scenarios(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Raise the soft `RLIMIT_NOFILE` to the hard limit. On macOS the hard limit is
/// reported as `RLIM_INFINITY` but the kernel silently caps at `OPEN_MAX`
/// (10240), so we cap the request at a safe value to avoid `EINVAL`.
#[cfg(unix)]
fn raise_fd_limit() -> Result<(), std::io::Error> {
    use std::io::Error;

    // Safe: `libc::rlimit` is plain data.
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // Safe: we pass a valid pointer to a zeroed struct.
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) };
    if rc != 0 {
        return Err(Error::last_os_error());
    }

    // macOS caps at OPEN_MAX (10240) even when rlim_max is RLIM_INFINITY.
    let target = {
        #[cfg(target_os = "macos")]
        {
            std::cmp::min(rlim.rlim_max, 10_240)
        }
        #[cfg(not(target_os = "macos"))]
        {
            rlim.rlim_max
        }
    };

    if rlim.rlim_cur >= target {
        return Ok(());
    }

    rlim.rlim_cur = target;
    // Safe: we pass a valid pointer to an initialised struct.
    let rc = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) };
    if rc != 0 {
        return Err(Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn raise_fd_limit() -> Result<(), std::io::Error> {
    Ok(())
}
