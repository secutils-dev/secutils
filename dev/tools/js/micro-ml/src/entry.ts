// Browser entry point for the micro-ml bundle. Re-exports the subset of
// upstream `micro-ml` (https://github.com/AdamPerlinski/micro-ml) used by
// `dev/tools/forecast.html`.
//
// The upstream npm package ships a Rust/WASM core (`micro_ml_core_bg.wasm`,
// ~145 KB) that its public API lazy-instantiates on first call. By default
// the browser path calls `await coreGlue.default()`, which does
// `fetch(new URL('./micro_ml_core_bg.wasm', import.meta.url))` -- that
// resolves against the bundle URL.
//
// We want a *single self-contained* `dist/micro-ml.js` (the
// `data-su-bundle="micro-ml"` contract requires it -- the responder body
// can't ship a sibling .wasm file). Vite's `build.assetsInlineLimit:
// Infinity` + the `vite-plugin-wasm` integration turns the `new URL(...,
// import.meta.url)` reference into a `data:` URL inline, so the upstream
// fetch path succeeds without any sidecar request. No source-level
// monkey-patching of the upstream glue is needed.
//
// We re-export only the functions the forecast tool actually uses; this
// keeps Rollup's tree-shaker honest and keeps the bundle from carrying
// dead classification / clustering / PCA code that the forecast tool will
// never reach. The whole bundle still includes the WASM blob (it's one
// monolithic .wasm; tree-shaking happens at the JS API layer, not inside
// the Rust core), but the JS surface shrinks meaningfully.

export {
  // Regression family used by the "Fit" picker.
  linearRegression,
  linearRegressionSimple,
  polynomialRegression,
  polynomialRegressionSimple,
  exponentialRegression,
  exponentialRegressionSimple,
  logarithmicRegression,
  powerRegression,
  // Smoothing window kinds.
  sma,
  ema,
  wma,
  // Forecasting + analysis helpers.
  trendForecast,
  findPeaks,
  findTroughs,
  // Statistical helpers used by the residual-z-score anomaly mode.
  residuals,
  // Seasonality auto-detect callout.
  detectSeasonality,
} from "micro-ml";
