import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// `dev/tools/js/micro-ml/` -- Vite build that produces `dist/micro-ml.js`,
// a single self-contained ESM module re-exporting the subset of `micro-ml`
// used by `dev/tools/forecast.html`. The build is consumed at deploy time
// by `dev/tools/deploy.ts`'s `data-su-bundle="micro-ml"` inliner; see
// `dev/tools/AGENTS.md` -> "Embedded JS bundles".
//
// How the WASM gets inlined:
//
//   Upstream `micro-ml@1.0.0`'s browser entry (`dist/index.js`) lazy-loads
//   its Rust core via:
//
//       const core = await import('./micro_ml_core-CYEMXCKP.js');
//       await core.default(); // wasm-pack glue
//
//   `core.default()`'s default behaviour is to fetch the WASM via
//   `new URL('./micro_ml_core_bg.wasm', import.meta.url)`. Vite already
//   handles that pattern: with `build.assetsInlineLimit: Infinity` it
//   rewrites the `new URL(...)` to a `data:application/wasm;base64,...`
//   constant inline in the bundle, and the wasm-pack glue's `fetch(C)`
//   call then resolves against the data URL (browser data: fetch is
//   synchronous-fast, no network). The bundle is fully self-contained;
//   no sibling .wasm file is required at runtime.
//
//   `inlineDynamicImports: true` collapses the chunked layout into a
//   single output file so the `data-su-bundle` inliner only has one
//   module body to splice into the HTML responder placeholder.
//
// Re-syncing against a newer `micro-ml`:
//
//   Verify after a bump that:
//     - `dist/micro-ml.js` still contains `data:application/wasm;base64,`
//       (= the WASM is inlined, no runtime fetch of a sidecar file).
//     - First-call latency from `await linearRegression(...)` is still
//       <50 ms on a warm laptop -- the bundle decodes the data URL once,
//       compiles the module, and caches the compiled instance.

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Throw-on-access stub for the Node-only `fs` / `url` / `path` imports that
// upstream `micro-ml/dist/index.js` performs inside its
// `typeof process == "node"` guard. The guard is `false` in the browser so
// the dynamic `await import('fs' | 'url' | 'path')` calls never execute --
// but Vite still resolves them statically and emits a warning per import
// (`Module "fs" has been externalized for browser compatibility`). Aliasing
// the three bare specifiers to a single stub silences the warnings without
// changing runtime behaviour; see `src/stubs/node-builtin.ts` for details.
const nodeBuiltinStub = resolve(__dirname, "src/stubs/node-builtin.ts");

export default defineConfig({
  resolve: {
    // Force the upstream package to resolve through its declared `import`
    // entry. `micro-ml`'s `exports` map only declares `import` + `types`;
    // making the condition list explicit is forward-compatible if Vite's
    // default ordering ever shifts.
    conditions: ["import", "browser", "module", "default"],
    alias: [
      // Order matters: exact-string matches via `find: "fs"` only fire on
      // bare specifiers, which is exactly what the upstream uses
      // (`await import('fs')`, not `'node:fs'`). We add the `node:` prefixed
      // variants too so the alias keeps working if the upstream ever
      // modernises its import strings.
      { find: "fs", replacement: nodeBuiltinStub },
      { find: "url", replacement: nodeBuiltinStub },
      { find: "path", replacement: nodeBuiltinStub },
      { find: "node:fs", replacement: nodeBuiltinStub },
      { find: "node:url", replacement: nodeBuiltinStub },
      { find: "node:path", replacement: nodeBuiltinStub },
    ],
  },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    minify: "esbuild",
    sourcemap: false,
    // Inline every asset (including the .wasm pulled in by upstream's
    // `new URL('./micro_ml_core_bg.wasm', import.meta.url)`) as a data
    // URL, so the produced ESM is fully self-contained.
    assetsInlineLimit: Number.MAX_SAFE_INTEGER,
    chunkSizeWarningLimit: 8 * 1024,
    lib: {
      entry: resolve(__dirname, "src/entry.ts"),
      formats: ["es"],
      fileName: () => "micro-ml.js",
    },
    rollupOptions: {
      external: [],
      output: {
        // Single output file -- `data-su-bundle` inlines exactly one
        // module body into the placeholder.
        inlineDynamicImports: true,
        exports: "named",
      },
    },
  },
});
