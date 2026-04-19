#!/usr/bin/env node

// Secutils JS-runtime performance analyzer.
//
// Reads the JSON report produced by `benches/js-runtime-perf`, compares the
// current numbers against the last entry in `.perf/history.jsonl`, prints a
// human-readable table, and appends the current report to history only when
// at least one tracked metric moved beyond `HISTORY_APPEND_THRESHOLD_PCT`.
// This keeps history sparse - one row per genuine movement - and prevents
// CI from committing a new history entry on every push when nothing changed.
//
// Always exits 0 (warn-only) - regressions become warnings, not CI failures.
// The harness is advisory: it exposes trends so humans can decide whether a
// change is acceptable.

import { appendFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// ── Types ────────────────────────────────────────────────

interface ScenarioResult {
  p50_us: number;
  p90_us: number;
  p99_us: number;
  max_us: number;
  mean_us: number;
  stddev_us: number;
  throughput_ops_per_sec: number;
  iterations: number;
  warmup: number;
  peak_rss_delta_kb: number;
}

interface EnvInfo {
  os: string;
  arch: string;
  cpuModel: string;
}

interface Report {
  timestamp: string;
  commitSha: string;
  commitMessage: string;
  env: EnvInfo;
  scenarios: Record<string, ScenarioResult>;
}

interface Thresholds {
  /** Max allowed p50 increase, as a percentage. */
  p50: number;
  /** Max allowed p99 increase, as a percentage. */
  p99: number;
  /** Max allowed throughput _decrease_, as a percentage (the absolute %). */
  throughput: number;
  /** Max allowed peak RSS delta increase, as a percentage. */
  peakRssDeltaKb: number;
}

interface Config {
  thresholds: Thresholds;
  scenarios: string[];
}

// ── Paths ────────────────────────────────────────────────

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const PERF_DIR = join(ROOT, '.perf');
const CONFIG_PATH = join(PERF_DIR, 'config.json');
const HISTORY_PATH = join(PERF_DIR, 'history.jsonl');
const INPUT_PATH = resolve(process.argv[2] ?? '/tmp/perf.json');

/// Minimum percentage change required for *any* tracked metric before we
/// append a new history entry. Tighter than run-to-run jitter we actually
/// care about, so anything under this is treated as noise and the previous
/// entry is kept as the canonical data point.
const HISTORY_APPEND_THRESHOLD_PCT = 0.1;

/// Per-scenario metrics considered when deciding whether to append. We
/// deliberately skip `iterations` / `warmup` (run configuration, not
/// measurements) and `mean_us` / `stddev_us` (derivable from the percentiles
/// and inherently noisier).
const TRACKED_METRICS = [
  'p50_us',
  'p90_us',
  'p99_us',
  'max_us',
  'throughput_ops_per_sec',
  'peak_rss_delta_kb',
] as const satisfies readonly (keyof ScenarioResult)[];

// ── Helpers ──────────────────────────────────────────────

function loadConfig(): Config {
  const defaults: Config = {
    thresholds: { p50: 15, p99: 20, throughput: 15, peakRssDeltaKb: 25 },
    scenarios: [],
  };
  if (!existsSync(CONFIG_PATH)) {
    return defaults;
  }
  try {
    const parsed = JSON.parse(readFileSync(CONFIG_PATH, 'utf8')) as Partial<Config>;
    return {
      thresholds: { ...defaults.thresholds, ...(parsed.thresholds ?? {}) },
      scenarios: parsed.scenarios ?? defaults.scenarios,
    };
  } catch {
    console.warn('⚠ Could not parse .perf/config.json, using defaults.');
    return defaults;
  }
}

function loadReport(): Report {
  if (!existsSync(INPUT_PATH)) {
    console.error(`Error: ${INPUT_PATH} not found. Run \`make perf\` first.`);
    process.exit(1);
  }
  try {
    return JSON.parse(readFileSync(INPUT_PATH, 'utf8')) as Report;
  } catch (err) {
    console.error(`Error: failed to parse ${INPUT_PATH}:`, err);
    process.exit(1);
  }
}

function loadHistory(): Report[] {
  if (!existsSync(HISTORY_PATH)) {
    return [];
  }
  const content = readFileSync(HISTORY_PATH, 'utf8').trim();
  if (!content) {
    return [];
  }
  return content
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as Report);
}

function appendReport(report: Report): void {
  mkdirSync(PERF_DIR, { recursive: true });
  appendFileSync(HISTORY_PATH, JSON.stringify(report) + '\n');
}

/// Describes the first detected material difference between `current` and
/// `previous`. Used both to decide whether to append and to surface a human
/// readable reason for the decision in the CLI output.
interface HistoryDelta {
  scenario: string;
  metric: string;
  previous: number;
  current: number;
  /** Absolute percent change, rounded to 2dp for display. */
  changePct: number;
}

/// Returns the first metric whose movement exceeds the append threshold, or
/// `null` if every tracked metric is within the threshold (i.e. the new run
/// is indistinguishable from the previous one at `HISTORY_APPEND_THRESHOLD_PCT`
/// resolution). Also returns a delta when a scenario appears or disappears,
/// since that's a structural change we always want to record.
function detectMaterialChange(current: Report, previous: Report): HistoryDelta | null {
  const curNames = new Set(Object.keys(current.scenarios));
  const prevNames = new Set(Object.keys(previous.scenarios));

  for (const name of curNames) {
    if (!prevNames.has(name)) {
      return { scenario: name, metric: '(new scenario)', previous: 0, current: 0, changePct: Infinity };
    }
  }
  for (const name of prevNames) {
    if (!curNames.has(name)) {
      return { scenario: name, metric: '(removed scenario)', previous: 0, current: 0, changePct: Infinity };
    }
  }

  for (const name of curNames) {
    const cur = current.scenarios[name];
    const prev = previous.scenarios[name];
    for (const metric of TRACKED_METRICS) {
      const a = prev[metric];
      const b = cur[metric];
      // If either side is zero, any non-zero value on the other side is
      // material. Treat 0 → 0 as unchanged. Avoids dividing by zero when
      // computing the percent change.
      if (a === 0 || b === 0) {
        if (a !== b) {
          return { scenario: name, metric, previous: a, current: b, changePct: Infinity };
        }
        continue;
      }
      const changePct = Math.abs(((b - a) / a) * 100);
      if (changePct > HISTORY_APPEND_THRESHOLD_PCT) {
        return { scenario: name, metric, previous: a, current: b, changePct };
      }
    }
  }

  return null;
}

function pctChange(current: number, previous: number): number | null {
  if (previous === 0) {
    return current === 0 ? 0 : null;
  }
  return ((current - previous) / previous) * 100;
}

function formatPct(pct: number | null): string {
  if (pct === null) {
    return '—';
  }
  const sign = pct > 0 ? '+' : '';
  return `${sign}${pct.toFixed(1)}%`;
}

function formatUs(us: number): string {
  if (us >= 1000) {
    return `${(us / 1000).toFixed(2)}ms`;
  }
  return `${us}µs`;
}

function formatThroughput(ops: number): string {
  if (ops >= 1000) {
    return `${(ops / 1000).toFixed(1)}k/s`;
  }
  return `${ops.toFixed(1)}/s`;
}

function formatKb(kb: number): string {
  if (kb >= 1024) {
    return `${(kb / 1024).toFixed(1)}MB`;
  }
  return `${kb}KB`;
}

// ── Reporting ────────────────────────────────────────────

interface Warning {
  scenario: string;
  metric: string;
  current: string;
  previous: string;
  change: string;
  threshold: number;
}

function analyze(current: Report, previous: Report | null, config: Config): Warning[] {
  const warnings: Warning[] = [];

  const scenarios = config.scenarios.length > 0 ? config.scenarios : Object.keys(current.scenarios);

  const shaShort = current.commitSha.slice(0, 7);
  const subject = current.commitMessage.length > 60 ? current.commitMessage.slice(0, 57) + '...' : current.commitMessage;
  const line = '─'.repeat(110);

  console.log('');
  console.log(`JS Runtime Perf Report – ${shaShort} (${subject})`);
  console.log(`Env: ${current.env.os}/${current.env.arch} – ${current.env.cpuModel}`);
  console.log(line);
  console.log(
    'Scenario'.padEnd(30) +
      'p50'.padStart(10) +
      'p99'.padStart(10) +
      'throughput'.padStart(14) +
      'rss'.padStart(10) +
      'Δp50'.padStart(10) +
      'Δp99'.padStart(10) +
      'Δops'.padStart(10) +
      'Δrss'.padStart(10),
  );
  console.log(line);

  for (const name of scenarios) {
    const cur = current.scenarios[name];
    if (!cur) {
      console.log(`${name.padEnd(30)}(missing)`);
      continue;
    }
    const prev = previous?.scenarios?.[name] ?? null;

    const dP50 = prev ? pctChange(cur.p50_us, prev.p50_us) : null;
    const dP99 = prev ? pctChange(cur.p99_us, prev.p99_us) : null;
    const dOps = prev ? pctChange(cur.throughput_ops_per_sec, prev.throughput_ops_per_sec) : null;
    const dRss = prev ? pctChange(cur.peak_rss_delta_kb, prev.peak_rss_delta_kb) : null;

    const row =
      name.padEnd(30) +
      formatUs(cur.p50_us).padStart(10) +
      formatUs(cur.p99_us).padStart(10) +
      formatThroughput(cur.throughput_ops_per_sec).padStart(14) +
      formatKb(cur.peak_rss_delta_kb).padStart(10) +
      formatPct(dP50).padStart(10) +
      formatPct(dP99).padStart(10) +
      formatPct(dOps).padStart(10) +
      formatPct(dRss).padStart(10);

    console.log(row);

    if (!prev) {
      continue;
    }

    if (dP50 !== null && dP50 > config.thresholds.p50) {
      warnings.push({
        scenario: name,
        metric: 'p50',
        current: formatUs(cur.p50_us),
        previous: formatUs(prev.p50_us),
        change: formatPct(dP50),
        threshold: config.thresholds.p50,
      });
    }
    if (dP99 !== null && dP99 > config.thresholds.p99) {
      warnings.push({
        scenario: name,
        metric: 'p99',
        current: formatUs(cur.p99_us),
        previous: formatUs(prev.p99_us),
        change: formatPct(dP99),
        threshold: config.thresholds.p99,
      });
    }
    if (dOps !== null && dOps < -config.thresholds.throughput) {
      warnings.push({
        scenario: name,
        metric: 'throughput',
        current: formatThroughput(cur.throughput_ops_per_sec),
        previous: formatThroughput(prev.throughput_ops_per_sec),
        change: formatPct(dOps),
        threshold: config.thresholds.throughput,
      });
    }
    if (dRss !== null && dRss > config.thresholds.peakRssDeltaKb) {
      warnings.push({
        scenario: name,
        metric: 'peak_rss_delta',
        current: formatKb(cur.peak_rss_delta_kb),
        previous: formatKb(prev.peak_rss_delta_kb),
        change: formatPct(dRss),
        threshold: config.thresholds.peakRssDeltaKb,
      });
    }
  }

  console.log(line);
  return warnings;
}

// ── Entry point ──────────────────────────────────────────

const config = loadConfig();
const current = loadReport();
const history = loadHistory();
const previous = history.at(-1) ?? null;

const warnings = analyze(current, previous, config);

if (!previous) {
  console.log('\nFirst run recorded – no comparison available.');
} else if (warnings.length > 0) {
  console.log('');
  for (const w of warnings) {
    console.log(
      `⚠ WARNING: ${w.scenario} ${w.metric} regressed from ${w.previous} → ${w.current} (${w.change}, threshold ±${w.threshold}%)`,
    );
  }
} else {
  console.log('\n✓ No regressions beyond configured thresholds.');
}

// Gate the append on material change so repeated runs against the same
// numbers don't keep growing history.jsonl (both locally and on CI).
const delta = previous ? detectMaterialChange(current, previous) : null;
if (!previous || delta) {
  appendReport(current);
  if (delta) {
    console.log(
      `\nAppended to ${HISTORY_PATH} (triggered by ${delta.scenario}/${delta.metric}: ` +
        `${delta.previous} → ${delta.current}, ±${delta.changePct.toFixed(2)}% > ±${HISTORY_APPEND_THRESHOLD_PCT}%).`,
    );
  } else {
    console.log(`\nAppended to ${HISTORY_PATH}`);
  }
} else {
  console.log(
    `\nAll tracked metrics within ±${HISTORY_APPEND_THRESHOLD_PCT}% of the previous run; ` +
      `history not updated.`,
  );
}
