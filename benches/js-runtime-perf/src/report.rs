//! JSON report shape. Intentionally flat so `scripts/analyze-perf.ts` and
//! `scripts/perf-report.html` can consume it without type gymnastics.

use crate::measure::ScenarioResult;
use anyhow::Context;
use serde::Serialize;
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Top-level JSON document written to `--output`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub timestamp: String,
    pub commit_sha: String,
    pub commit_message: String,
    pub env: EnvInfo,
    pub scenarios: BTreeMap<String, ScenarioResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvInfo {
    pub os: &'static str,
    pub arch: &'static str,
    pub cpu_model: String,
}

impl Report {
    pub fn new() -> Self {
        let (commit_sha, commit_message) = git_info();
        Self {
            timestamp: current_timestamp_iso8601(),
            commit_sha,
            commit_message,
            env: EnvInfo {
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
                cpu_model: detect_cpu_model(),
            },
            scenarios: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, name: &str, result: ScenarioResult) {
        self.scenarios.insert(name.to_string(), result);
    }

    pub fn write(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let serialised =
            serde_json::to_string_pretty(self).context("serialising report to JSON")?;
        fs::write(path, serialised).with_context(|| format!("writing {}", path.display()))
    }
}

fn git_info() -> (String, String) {
    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let msg = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    (sha, msg)
}

fn current_timestamp_iso8601() -> String {
    // Minimal ISO-8601 formatter that avoids pulling `time` / `chrono` into
    // the harness crate just for a single call.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // days_from_unix_epoch → y/m/d using a standard civil-from-days conversion.
    let days = (now / 86_400) as i64;
    let secs_of_day = (now % 86_400) as u32;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        secs_of_day / 3600,
        (secs_of_day / 60) % 60,
        secs_of_day % 60
    )
}

/// Howard Hinnant's civil-from-days algorithm. Good for [−5 877 641, 5 881 580].
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn detect_cpu_model() -> String {
    // Best-effort. Matches whatever is easy to read on Linux CI runners.
    #[cfg(target_os = "linux")]
    if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                if let Some(idx) = rest.find(':') {
                    return rest[idx + 1..].trim().to_string();
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    if let Ok(out) = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        && out.status.success()
    {
        return String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    "unknown".to_string()
}
