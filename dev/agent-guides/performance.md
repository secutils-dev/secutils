# Runtime performance and Web UI bundle tracking

Run commands from the Secutils root. Retrack owns its own harness and guide.
Read source scenario definitions before interpreting results because runtime architecture changes can alter their meaning.

## JS runtime harness

The harness in `benches/js-runtime-perf/` links the real `secutils::js_runtime::JsRuntime`.
Read `src/js_runtime.rs`, `src/js_runtime/worker_pool.rs`, and `src/js_runtime/op_proxy_request.rs` for the current execution architecture.
Workers retain Tokio runtimes and proxy clients. Script executions create fresh V8 isolates.
The cold-start scenario name does not establish that every execution creates a thread or HTTP client.

Use `make help` and the harness CLI for flags/defaults.
Read `benches/js-runtime-perf/src/scenarios.rs` for default and on-demand scenarios.

```bash
make perf
make perf ANALYZE=1 PERF_ITERATIONS=20 PERF_WARMUP=5
make perf PERF_OUTPUT=/tmp/perf-baseline.json
make perf-analyze
make perf-report
```

`make perf` produces the report without history changes.
`ANALYZE=1` compares against `.perf/history.jsonl` and can append a history entry.
When comparison/history recording is intended, use `ANALYZE=1`.
Benchmark regressions produce advisory warnings. Execution or analysis failures still require investigation.
Compare platform-matched results through `env.os`, `env.arch`, and `env.cpuModel`.

### Resident isolates

`resident_isolates` is a manual capacity probe outside the default `ALL` scenarios and tracked history.
Name it explicitly and divide `peak_rss_delta_kb` by concurrency for marginal resident cost.
Use `RESIDENT_PARK_MS` to extend the park window for large concurrency.
Read the scenario for its default and clamp.

```bash
cargo run -p js-runtime-perf -- --scenarios resident_isolates --concurrency 1000
SECUTILS_JS_WORKERS=1000 SECUTILS_JS_MAX_WORKERS=1000 RESIDENT_PARK_MS=12000 \
  cargo run --release -p js-runtime-perf -- --scenarios resident_isolates --concurrency 1000
```

The second command prewarms workers to distinguish isolate cost from thread startup.

### Scenarios and history

For a default scenario, register its module, `run` dispatcher, `ALL` entry, and `.perf/config.json` entry.
For an on-demand probe, use `ON_DEMAND` and the dispatcher, outside `ALL` and tracked configuration.
Completion requires the scenario to run and produce the expected report fields.

Read `scripts/analyze-perf.ts` for tracked metrics and `HISTORY_APPEND_THRESHOLD_PCT`.
History appends only on material movement, scenario changes, or structural zero/nonzero transitions.
Read `.perf/config.json` for advisory thresholds.
Positive latency/RSS deltas and negative throughput deltas indicate regressions.

CI records reports and can commit material history changes with `[skip ci]`.
The root perf and bundle jobs share `dev/scripts/commit-ci-history.sh` and can race.
Keep each job's history file distinct and append-only so fetch/rebase/retry remains valid.

## Web UI bundles

Before bundle analysis, build Web UI output:

```bash
npm --prefix components/secutils-webui run build
npm --prefix components/secutils-webui run analyze
```

`build:analyze` also cleans before build and analysis.
The analyzer reads `dist/`, compares tracked bundles, and appends material history changes.
Regressions are advisory. Missing output is an analysis failure.

For a new lazy-loaded workspace route, add its pattern to `components/secutils-webui/.bundlesize/config.json`.
Otherwise, its output falls into untracked other chunks.
Completion requires the new bundle to appear in the tracked analysis output.
Read the configuration and analyzer for budgets, patterns, and append thresholds.
