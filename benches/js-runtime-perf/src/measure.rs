//! Measurement primitives: a hdrhistogram-backed latency recorder plus a
//! portable peak-RSS probe.
//!
//! We deliberately avoid criterion/divan here. Scenarios run at millisecond
//! scale, one script execution per observation is already a stable unit, and
//! we want direct control over warmup/iteration counts and the eventual JSON
//! shape (see `report.rs`). `hdrhistogram` provides well-tested percentile
//! math; everything else is plain `std`.

use anyhow::Context;
use hdrhistogram::Histogram;
use serde::Serialize;
use std::time::{Duration, Instant};

/// Single scenario measurement summary written into the JSON report.
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioResult {
    pub p50_us: u64,
    pub p90_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
    pub mean_us: u64,
    pub stddev_us: u64,
    pub throughput_ops_per_sec: f64,
    pub iterations: u64,
    pub warmup: u64,
    pub peak_rss_delta_kb: i64,
}

/// Records latency samples and wall-clock throughput for a single scenario.
pub struct Recorder {
    histogram: Histogram<u64>,
    wall_clock: Duration,
    iterations: u64,
    warmup: u64,
    rss_start_kb: i64,
    rss_peak_kb: i64,
}

impl Recorder {
    pub fn new(iterations: u64, warmup: u64) -> anyhow::Result<Self> {
        // Tracks 1 µs … 60 s at 3 significant digits (~0.1% resolution).
        let histogram = Histogram::<u64>::new_with_bounds(1, 60_000_000, 3)
            .context("failed to build hdrhistogram")?;
        let rss_start_kb = current_rss_kb();

        Ok(Self {
            histogram,
            wall_clock: Duration::ZERO,
            iterations,
            warmup,
            rss_start_kb,
            rss_peak_kb: rss_start_kb,
        })
    }

    /// Time a single measurement and add it to the histogram. Sets the wall
    /// clock and updates the peak-RSS watermark.
    pub fn observe(&mut self, duration: Duration) -> anyhow::Result<()> {
        let us = duration.as_micros().min(u64::from(u32::MAX) as u128) as u64;
        self.histogram
            .record(us.max(1))
            .context("histogram record failed")?;
        self.wall_clock += duration;
        self.rss_peak_kb = self.rss_peak_kb.max(current_rss_kb());
        Ok(())
    }

    pub fn finalise(self) -> ScenarioResult {
        let throughput = if self.wall_clock.as_secs_f64() > 0.0 {
            self.iterations as f64 / self.wall_clock.as_secs_f64()
        } else {
            0.0
        };

        ScenarioResult {
            p50_us: self.histogram.value_at_quantile(0.50),
            p90_us: self.histogram.value_at_quantile(0.90),
            p99_us: self.histogram.value_at_quantile(0.99),
            max_us: self.histogram.max(),
            mean_us: self.histogram.mean() as u64,
            stddev_us: self.histogram.stdev() as u64,
            throughput_ops_per_sec: throughput,
            iterations: self.iterations,
            warmup: self.warmup,
            peak_rss_delta_kb: (self.rss_peak_kb - self.rss_start_kb).max(0),
        }
    }
}

/// Stopwatch helper so callers don't have to import `Instant` directly.
pub fn now() -> Instant {
    Instant::now()
}

/// Reads the process's *current* resident set size in kilobytes.
///
/// We deliberately avoid `getrusage(RUSAGE_SELF).ru_maxrss` here: that field is
/// the process-lifetime high-water mark, so once the warmup loop has pushed RSS
/// to its steady state the value stops moving and every scenario's
/// `peak - start` delta collapses to zero. Sampling the current RSS and taking
/// the max of those samples gives a real "growth during measurement" signal on
/// every platform.
///
/// Platforms without a supported probe fall back to zero, which is acceptable
/// for the "warn-only" reporting mode - RSS just shows up as a constant 0 delta
/// in the report.
#[cfg(target_os = "linux")]
fn current_rss_kb() -> i64 {
    // `/proc/self/statm` columns (all in page units): size, resident, shared,
    // text, lib, data, dt. We only need the second column.
    let statm = match std::fs::read_to_string("/proc/self/statm") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut fields = statm.split_whitespace();
    fields.next();
    let resident_pages: i64 = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let page_size_kb = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } / 1024;
    resident_pages.saturating_mul(page_size_kb.max(0))
}

#[cfg(target_os = "macos")]
fn current_rss_kb() -> i64 {
    // Mach `task_info(MACH_TASK_BASIC_INFO)` exposes the current `resident_size`
    // in bytes. `libc` doesn't ship typed bindings on Darwin, so we wire them up
    // by hand; the struct layout matches `<mach/task_info.h>`.
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [i32; 2],
        system_time: [i32; 2],
        policy: i32,
        suspend_count: i32,
    }

    unsafe extern "C" {
        fn mach_task_self() -> u32;
        fn task_info(
            target_task: u32,
            flavor: u32,
            task_info_out: *mut i32,
            task_info_count: *mut u32,
        ) -> i32;
    }

    const MACH_TASK_BASIC_INFO: u32 = 20;
    const KERN_SUCCESS: i32 = 0;

    unsafe {
        let mut info: MachTaskBasicInfo = std::mem::zeroed();
        let mut count: u32 =
            (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<i32>()) as u32;
        let kr = task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as *mut i32,
            &mut count,
        );
        if kr != KERN_SUCCESS {
            return 0;
        }
        (info.resident_size / 1024) as i64
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn current_rss_kb() -> i64 {
    0
}
