---
name: forecast
description: >-
  Fit a trendline to numeric data, project N periods into the future, smooth
  noisy series, or hunt for anomalies using the Secutils.dev Forecast tool.
  Hand the user https://tools.secutils.dev/forecast (optionally with
  ?example=<id> to deep-link a curated dataset, or with the user's own data
  encoded in the URL fragment for one-click preload), tell them to pick a
  Fit / Smoothing / Forecast / Anomaly mode on the right and watch the
  chart update. Trigger when the user asks to "fit a trendline", "forecast
  the next N months", "find anomalies in this metric", "smooth this noisy
  series", "is this growth linear or exponential", "when will we hit X
  users", or anything that names secutils.dev/forecast.
---

# Forecast (Secutils.dev)

In-browser trend forecaster powered by [`micro-ml`](https://github.com/AdamPerlinski/micro-ml)
(WASM, ~56 KB gzipped on its own, fully self-contained in the page bundle).
Paste a numeric series, choose a fit / smoothing / forecast horizon /
anomaly mode, and read the chart + the structured results panel.

Five kinds of jobs the tool handles well:

1. **Trend fit + N-period forecast** -- pick `Fit = Auto (best R²)` or a
   specific family (linear / polynomial / exponential / logarithmic /
   power), set `Forecast horizon`. The Results panel surfaces the
   equation, R², slope/intercept, and (for exponential) doubling time.
2. **Anomaly detection** -- `Anomalies = Peaks / Troughs / Residual
   z-score`. Z-score uses the residuals against the chosen fit (or, when
   `Fit = None`, against the smoothed baseline or series mean) and a
   tunable threshold.
3. **Smoothing** -- `Smoothing = SMA / EMA / WMA` with a window slider.
   Useful as a baseline or just to denoise a series before screenshotting.
4. **Seasonality** -- check `Auto-detect seasonality`; the Results panel
   reports the detected period + strength from
   `micro-ml.detectSeasonality(...)`.
5. **Walk-forward backtest + conformal prediction band** -- check
   `Walk-forward backtest`. The tool holds out the last K = round(n × pct)
   points, re-fits every family on each expanding-window fold, and
   produces (a) a leaderboard ranking models by out-of-sample MAE (with a
   naive last-value baseline for free), (b) a calibrated conformal
   prediction interval replacing the legacy in-sample ±2σ band on the
   chart, and (c) an honest signal for `Auto`: when backtest is on, the
   tournament picks the model with the lowest MAE rather than the best
   adjusted R². When the naive baseline beats every regression the
   leaderboard flags it loudly -- a strong signal that the series has no
   exploitable trend and any forecast is overfitting noise.

## Inputs

| Field   | Type   | Default  | Notes                                                                              |
|---------|--------|----------|------------------------------------------------------------------------------------|
| `data`  | string | required | Numbers one per line, two-column CSV (`x,y`), or a JSON array (`[1,2,3]` or `[[x,y]...]`). The first row may be a header. |

Controls (all optional; tool runs as soon as `data` parses):

| Control            | Values                                                       | Notes                                                                 |
|--------------------|--------------------------------------------------------------|-----------------------------------------------------------------------|
| `Fit`              | `none` / `auto` / `linear` / `poly2..4` / `exponential` / `logarithmic` / `power` | `auto` tries every family and keeps the highest finite **adjusted** R² (falls back to plain R² when n &le; p + 1). When **`Walk-forward backtest`** is on, `auto` instead picks the leaderboard winner -- the family with the lowest out-of-sample 1-step-ahead MAE -- which is the stronger selection criterion. The Fit card discloses which selection was used. |
| `Smoothing`        | `none` / `sma` / `ema` / `wma`                               | Plus a `Smoothing window` slider (2-30). The window slider is disabled when `Smoothing = none`. |
| `Forecast horizon` | 0-50                                                         | Forecast is generated from the chosen fit. Disabled when `Fit = None`. |
| `Anomalies`        | `none` / `peaks` / `troughs` / `zscore`                      | Z-score threshold slider (1.0-5.0); default 2.5. The slider is disabled in every mode except `zscore`. |
| `Auto-detect seasonality` | checkbox                                              | Surfaces the detected period + strength as a callout card.            |
| `Walk-forward backtest`   | checkbox                                              | Enables the leaderboard + conformal prediction band. Requires `n &ge; 12`. Triggers two extra sliders (below).                                                       |
| `Hold-out (% of n)`       | 10-40                                                 | Percentage of the trailing series used for the expanding-window backtest. Hard-capped at `K = 40` folds regardless of percentage (recompute stays &lt; 2 s).         |
| `Coverage target`         | 50-99                                                 | Target coverage `1 - α` for the conformal prediction interval. Width = `ceil((K + 1) × (1 - α))`-th order statistic of the winning family's `|residual|` on the K folds. |

## Examples catalogue

The tool ships twelve curated examples in a **Load example &#x25BE;**
dropdown. Deep-link any of them with `?example=<id>`; the slug is stable
across deploys.

| `?example=<id>`        | Group                       | What it demonstrates                                                                                                       |
|------------------------|-----------------------------|----------------------------------------------------------------------------------------------------------------------------|
| `monthly-sales`        | Forecasting                 | Linear growth, 3-month forecast, slope + R² readout.                                                                       |
| `user-growth`          | Forecasting                 | Exponential signups; surfaces `doublingTime()`.                                                                            |
| `training-completion`  | Forecasting                 | Logarithmic curve approaching 100% (diminishing returns).                                                                  |
| `cloud-bill`           | Forecasting                 | Polynomial deg-2 + EMA smoothing, 6-month projection.                                                                      |
| `api-latency`          | Anomaly detection           | 90 daily samples + EMA baseline; residual z-score flags 3 incident days.                                                   |
| `login-attempts`       | Anomaly detection           | 168 hourly counts with a credential-stuffing burst; peaks detection.                                                       |
| `ci-builds`            | Anomaly detection           | 120 build durations with slow drift + outlier weeks; z-score @ 2.0.                                                        |
| `compound-interest`    | Finance                     | $10k at 5% APY compounded monthly; exponential fit + `doublingTime()` reads the Rule of 72.                                |
| `inflation`            | Finance                     | 31 years of US CPI YoY %; residual z-score (1.8) flags the 2021-2022 post-pandemic spike.                                  |
| `webhook-traffic`      | Smoothing &amp; seasonality | 7 days x 24 hours with a daily rhythm and Saturday dip; seasonality auto-detect.                                           |
| `backtest-trap`        | Backtest &amp; conformal    | 80-step Gaussian random walk. The leaderboard exposes that no regression beats "tomorrow = today" -- the naive-beats-all banner fires. |
| `conformal-coverage`   | Backtest &amp; conformal    | 100-point linear series with 10% outlier shocks (heavy-tailed residuals); conformal width widens past 2σ to hit 90% coverage.    |

Example URL: `https://tools.secutils.dev/forecast?example=api-latency`.

## Outputs

After the chart paints, the Results panel shows up to six cards:

- **Fit** -- model kind, `Selected by` (`Auto -> backtest MAE` when backtest is on, otherwise `Auto -> adjusted R²`), R², adjusted R² (when defined; n > p + 1), slope/intercept (linear), `a`/`b`/`doublingTime` (exponential), and the equation string from `model.toString()`. For `logarithmic` / `power` fits the tool transparently shifts the x-axis when the supplied `x` is non-positive (otherwise the upstream fails with `All x values must be positive`); the shift amount is reflected in the kind label (e.g. `logarithmic (x shifted by +1)`), in the equation card as `where x' = x + 1`, and is exposed as `xShift` in `Copy JSON`. `predict()` re-applies the shift internally, so callers always pass the original x values.
- **Forecast** -- the first 10 projected values, plus a prediction band. When `Walk-forward backtest` is OFF the band is the legacy ±2σ heuristic on in-sample residuals (explicitly disclosed as **not** a calibrated confidence interval). When backtest is ON the band is a **calibrated conformal interval** of width `q` derived from the leaderboard winner's hold-out residuals; under exchangeability of residuals this guarantees coverage ≥ `1 - α` for the next out-of-sample point (Vovk 2005; Angelopoulos &amp; Bates 2021, arXiv:2107.07511).
- **Backtest leaderboard** -- one row per family + a naive last-value baseline. Columns: `Model`, `In-sample R²`, `Adj. R²`, `MAE`, `RMSE`, `MAPE` (or `—` when any actual is ≤ 0), and `Coverage` (empirical % of hold-out points falling within the winner's conformal band q; flagged red if it falls more than 5 pp below the target). The winner row is highlighted; if the naive baseline beats every regression, a "naive beats all" banner fires.
- **Anomalies** -- indices and values of flagged points (first 8 listed; chart shows all).
- **Seasonality** -- detected period + strength.
- **Warnings** -- any micro-ml call that failed (e.g. exponential on data containing zero / negative values, backtest skipped because n &lt; 12), surfaced inline.

Two **Copy** actions in the chart's panel bar:

- `Copy CSV` -- columns `idx,x,y[,smoothed][,fit][,forecast]`.
- `Copy JSON` -- the full structured result including residual band, anomaly indices, seasonality, **and the full backtest object** (`{ holdoutK, foldStart, coverage, alpha, conformalWidth, winnerKind, leaderboard: [{ kind, params, inSampleR2, adjR2, mae, rmse, mape, empiricalCoverage }] }`). Per-fold `predicted` / `actual` arrays are deliberately omitted from the export -- they're re-derivable from `foldStart + holdoutK` and would quadruple the payload.

## Wire format (URL state)

The Share button copies `tools.secutils.dev/forecast#<encoded>` with the
canonical Secutils.dev format:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8(JSON) |
```

The JSON payload shape is:

```json
{
  "d": "<data text exactly as typed>",
  "c": {
    "fit": "linear",
    "smooth": "ema",
    "window": 7,
    "horizon": 0,
    "anomaly": "zscore",
    "z": 2.5,
    "seasonality": false,
    "backtest": false,
    "backtestHoldout": 20,
    "backtestCoverage": 90
  }
}
```

The three `backtest*` fields are optional from the agent's perspective:
when omitted on a round-trip, `applyPreset` defaults them to `false / 20 /
90` so older share URLs (and skills authored before this section existed)
keep working unchanged.

Pipeline: `JSON.stringify` -> UTF-8 -> `deflate-raw` -> prepend 4-byte LE
u32 of the **uncompressed** length -> base64url. The fragment is **never**
sent to the Secutils.dev server.

## How to direct the user

If the user already has data, encode + hand them a pre-filled URL. From
any machine with Node &ge; 18:

```bash
node -e '
const zlib = require("node:zlib");
const payload = JSON.stringify({
  d: process.argv[1],
  c: { fit: "auto", smooth: "none", window: 5, horizon: 3, anomaly: "none", z: 2.5, seasonality: false, backtest: true, backtestHoldout: 20, backtestCoverage: 90 },
});
const utf8 = Buffer.from(payload, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/forecast#" + enc);
' '42000
45000
48000
52000
55000
58000'
```

Pass the data as a single argv (single-quoted) so newlines survive intact.

If the user just wants to *see* the tool with realistic data, hand them a
`?example=<id>` URL from the catalogue above instead -- it's shorter and
the slug is human-readable.

## Inline alternative (no tool needed)

If the user is comfortable doing it themselves, both
[`micro-ml`](https://www.npmjs.com/package/micro-ml) (`npm i micro-ml`)
and pure-JS alternatives like `simple-statistics` give them the same
math directly:

```js
import { linearRegression, trendForecast } from 'micro-ml';

const sales = [42000, 45000, 48000, 52000, 55000, 58000];
const model = await linearRegression(sales.map((_, i) => i), sales);
console.log(`Slope: ${model.slope.toFixed(0)}/month, R² ${model.rSquared.toFixed(3)}`);

const f = await trendForecast(sales, 3);
console.log('Next 3 months:', f.getForecast());
```

Use the tool when the user wants a chart and a shareable URL; use the
library directly when they want to embed the math in their own code.

## After producing

If you've handed over the URL, that's the whole interaction -- the user
takes it from there in the browser. No follow-up encoding required.

## Caveats

- **Browser-only.** All math runs client-side inside a WASM module bundled
  with the page; numeric data never leaves the browser. The share URL's
  fragment (`#...`) is **never** sent to the Secutils.dev server -- safe
  for content the user wouldn't want logged, but anyone who receives the
  link can read the data.
- **Confidence band is informational only.** The shaded forecast band is
  ±2 standard deviations of the fit's residuals. It is **not** a
  calibrated confidence or prediction interval; do not present it as one
  in a regulated context. For real CIs, point the user at R / Python /
  scikit-learn.
- **URL fragment ceiling.** Practical browser/server URL limits land
  around 8 KB; very long series (book-length CSVs) won't fit in a share
  link. The eight catalogue examples are all sized to fit (after
  deflate). Suggest the user download CSV/JSON and share the file
  instead if the encoded length warning fires.
- **Exponential / logarithmic / power regressions require positive
  inputs** (different subsets each). Logarithmic and power additionally
  need `x > 0`; the tool auto-shifts the x-axis when the supplied x has
  `min(x) <= 0` (typical for the default index `x = 0, 1, 2, ...`), so
  the fit runs and the shift is reflected in the kind label / equation
  card / `xShift` JSON field. Exponential fits accept x = 0 (since
  `e^0 = 1`) but still require `y > 0`; that case isn't shifted -- if y
  contains zero or negative values, the family is skipped (`auto`
  quietly falls back, explicit fit kinds surface a Warning).
- **Walk-forward backtest needs n &ge; 12.** Below that, the leaderboard
  has too few hold-out points for the conformal quantile (and the
  per-fold fits) to be statistically meaningful. The toggle is silently
  ignored on shorter series and a "Backtest skipped" Warning surfaces.
- **Conformal coverage assumption.** The interval's coverage guarantee
  holds under **exchangeability** of residuals -- much weaker than the
  i.i.d. normal assumption the legacy ±2σ band makes, but still a real
  assumption. It can fail when the data has a strong trend break inside
  the hold-out window (the post-break residuals are no longer exchangeable
  with pre-break ones), or when there's heavy autocorrelation (one big
  miss makes the next miss more likely). In both cases the empirical
  coverage column on the leaderboard will visibly fall below the target;
  treat that as a red flag, not a bug.
- **Backtest cost.** Hard-capped at K = 40 folds × 7 fit families = 280
  WASM fits per recompute, ~1-2 s on a warm laptop. Above n ≈ 1000 the
  recompute is still snappy because the cap kicks in. For an extremely
  long series (10⁵+ points) the lazy debounce keeps things responsive.
- **Sub-millisecond on the typical 100-point series.** The WASM bundle
  decodes the first time you trigger a recompute (~10-30 ms one-shot);
  every subsequent control change is near-instant.
- **Not multivariate.** Single series only (X/Y at most). If the user
  asks for clustering / PCA / kNN / multivariate forecasting, name
  [`micro-ml`](https://github.com/AdamPerlinski/micro-ml) directly and
  point them at the library README.
