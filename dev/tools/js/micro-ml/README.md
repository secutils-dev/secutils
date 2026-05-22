# `@secutils-dev/micro-ml-browser`

Browser-compatible Vite build of [`micro-ml`](https://github.com/AdamPerlinski/micro-ml).
Produces a single self-contained ESM module at `dist/micro-ml.js` that is
inlined into `dev/tools/forecast.html` at deploy time by
`dev/tools/deploy.ts`'s `data-su-bundle="micro-ml"` mechanism.

Not published. Internal to this repo. See
[`dev/tools/AGENTS.md`](../../AGENTS.md) -> "Embedded JS bundles" for the
inliner contract.

## Build

```bash
npm ci
npm run build
# => dist/micro-ml.js
```

`make tools-bundles` from the repo root builds this (and any future
bundles) in one go. `make deploy-tools` builds it on demand if
`dist/micro-ml.js` is missing or older than the sources here.

## Pinned upstream versions

| dep         | version    | notes                                                   |
|-------------|------------|---------------------------------------------------------|
| `micro-ml`  | `1.0.0`    | exact pin -- we patch its WASM-loader path via the plugin in `vite.config.ts` |
| `vite`      | `^7.1.13`  | bundler                                                 |
| (vendored) `micro_ml_core_bg.wasm` | (~145 KB) | shipped inside `micro-ml` itself        |

The bundle is shipped to the responder body in the `gzip-base64` encoding
variant of `data-su-bundle` (~250 KB gzipped, ~340 KB base64'd). The
forecast HTML loader reverses both steps via `DecompressionStream('gzip')`
on first use.

## How the WASM gets inlined

Upstream `micro-ml@1.0.0`'s browser entry lazy-loads its Rust core via:

```js
const core = await import('./micro_ml_core-CYEMXCKP.js');
await core.default(); // fetches micro_ml_core_bg.wasm via new URL(..., import.meta.url)
```

The `data-su-bundle` contract requires a **single self-contained** file --
the responder body cannot ship a sibling `.wasm`. So `vite.config.ts`'s
`microMlInlineWasmPlugin`:

1. Reads `node_modules/micro-ml/dist/micro_ml_core_bg.wasm` at build time.
2. Exposes the bytes via the virtual `virtual:micro-ml-wasm-bytes`
   specifier (default export is a base64-decoded `Uint8Array`).
3. Rewrites upstream `dist/index.js` to call
   `core.initSync({ module: <bytes> })` directly, bypassing the
   `default()` fetch path. The Node branch is left untouched; it's
   harmless dead code in the browser bundle.

If a future `micro-ml` release reshapes the minified source, the
`ELSE_BRANCH_RE` regex stops matching and the build **throws** at the
`transform` stage. Re-inspect `node_modules/micro-ml/dist/index.js` and
update the regex.

## Re-syncing against a newer `micro-ml`

1. Bump the pin in `package.json` (exact version, not a range).
2. `npm install`.
3. `npm run build`. If it throws "could not find the browser branch of
   micro-ml's lazy WASM initialiser", open
   `node_modules/micro-ml/dist/index.js` and grep for `else await
   ...default()` -- update `ELSE_BRANCH_RE` to match. The right-hand
   replacement (`initSync({ module })`) does not change because
   wasm-pack's `initSync({ module })` API is stable.
4. Verify the bundle still inlines the WASM:
   `grep -c '"data:application/wasm' dist/micro-ml.js` should print `0`
   (we don't go through a data: URL; we feed bytes straight to
   `initSync`), and `grep -c 'fetch' dist/micro-ml.js` should be 0
   (no runtime fetch of the WASM).
5. Smoke-test by loading the forecast tool and clicking a sample
   dataset: the first chart paint should happen within ~50 ms after the
   bundle is decoded.

## Runtime caveats

1. **First-use cost.** The bundle decodes a base64'd WASM blob and calls
   `WebAssembly.Module()` synchronously on first ML invocation. On a
   modern laptop this is ~10-30 ms. The forecast tool only loads the
   bundle on the first user action, so a search-result visitor never pays
   it.
2. **Worker version is not bundled.** Upstream ships a `micro-ml/worker`
   subpath that wraps the API in a Web Worker. The forecast tool runs on
   the main thread (sub-millisecond on typical 200-point series), so we
   don't bundle the worker variant.
3. **The Node branch is dead code in the browser bundle.** It's preserved
   so that running this bundle under Node would still work (defensive,
   not load-bearing). esbuild's minifier does not fully drop it because
   the `typeof globalThis.process` guard is opaque at compile time.
