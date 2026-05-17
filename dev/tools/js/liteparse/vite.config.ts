import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

// `dev/tools/js/liteparse/` — Vite build that produces `dist/liteparse.js`,
// a single self-contained ESM module re-exporting `@llamaindex/liteparse`'s
// public API rewired to run in a browser tab. The build is consumed at deploy
// time by `dev/tools/deploy.ts`'s `data-su-bundle="liteparse"` inliner; see
// `dev/tools/AGENTS.md` -> "Embedded JS bundles".
//
// What we redirect (and why):
//
//   engines/pdf/pdfium-renderer.js   -> src/stubs/pdfium-renderer.ts
//       Upstream uses @hyzyla/pdfium (Node-only, native bindings). The stub
//       provides the same surface (loadDocument, renderPageToBuffer, ...)
//       implemented with PDF.js + OffscreenCanvas.
//
//   engines/pdf/pdfjsImporter.js     -> src/stubs/pdfjsImporter.ts
//       Upstream uses a Node fs/url import dance to load the vendored pdf.mjs
//       and resolve worker / cmap directories on disk. The stub imports
//       pdf.mjs through a Vite alias that points at the same vendored file
//       inside the installed @llamaindex/liteparse package (no second copy)
//       and Blob-URLs the worker source (bundled inline via `?raw`).
//
//   engines/ocr/http-simple.js       -> src/stubs/http-simple.ts
//       Upstream POSTs page images to a remote OCR HTTP server. No fetch
//       footprint in the bundle, no need for `form-data`/`axios`.
//
//   conversion/convertToPdf.js       -> src/stubs/convertToPdf.ts
//       Upstream shells out to libreoffice / soffice for office docs. We only
//       support PDFs in the browser; the stub no-ops + throws on use.
//
//   processing/gridVisualizer.js     -> src/stubs/gridVisualizer.ts
//   processing/gridDebugLogger.js    -> src/stubs/gridDebugLogger.ts
//       Debug-only visualisation paths that pull in canvas/svg-on-disk
//       deps. No-ops.
//
//   file-type (npm)                  -> src/stubs/file-type.ts
//       Replaces the full file-type package (15 KB+ ESM, depends on `peek-readable`
//       which expects Node streams) with a 12-line magic-byte sniffer for PDFs.
//
//   node:fs, node:fs/promises,       -> src/stubs/empty.ts
//   node:child_process, node:os,
//   node:stream, node:tty
//       Hard-stub Node built-ins so a stray non-browser import path surfaces
//       as a clear "[liteparse-browser] accessed stubbed Node module property"
//       error at runtime instead of a baffling Vite resolve failure.
//
//   node:path, node:url              -> src/stubs/node-path.ts / node-url.ts
//       Liteparse's grid / bbox code does call `path.basename` and friends
//       on log labels, so these need to be working shims rather than empty.

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const require = createRequire(import.meta.url);

// Locate the installed @llamaindex/liteparse package root so we can alias
// `virtual:liteparse-pdfjs/*` to the pdf.mjs / pdf.worker.mjs files it ships
// under `dist/src/vendor/pdfjs/`. We go through `./package.json` (the only
// subpath the package's `exports` map publishes besides `.`), because the
// main entry's `exports` field declares an `import` condition only and
// `require.resolve()` from this CJS-style helper would throw
// `ERR_PACKAGE_PATH_NOT_EXPORTED`. Using a resolved-then-derived path
// keeps the bundle building under pnpm / npm workspaces with hoisting.
const liteparsePkgJson = require.resolve("@llamaindex/liteparse/package.json");
const liteparsePkgRoot = dirname(liteparsePkgJson);
const vendoredPdfJs = resolve(
  liteparsePkgRoot,
  "dist/src/vendor/pdfjs/pdf.mjs",
);
const vendoredPdfWorker = resolve(
  liteparsePkgRoot,
  "dist/src/vendor/pdfjs/pdf.worker.mjs",
);

const stub = (file: string) => resolve(__dirname, "src/stubs", file);

// `endsWithRedirect` is keyed off a substring of the resolved import path
// (always POSIX-style after Vite's resolution); we match against the tail so
// we don't accidentally match anything outside @llamaindex/liteparse.
const FILE_REDIRECTS: { suffix: string; target: string }[] = [
  { suffix: "/engines/pdf/pdfium-renderer.js", target: stub("pdfium-renderer.ts") },
  { suffix: "/engines/pdf/pdfjsImporter.js", target: stub("pdfjsImporter.ts") },
  { suffix: "/engines/ocr/http-simple.js", target: stub("http-simple.ts") },
  { suffix: "/conversion/convertToPdf.js", target: stub("convertToPdf.ts") },
  { suffix: "/processing/gridVisualizer.js", target: stub("gridVisualizer.ts") },
  { suffix: "/processing/gridDebugLogger.js", target: stub("gridDebugLogger.ts") },
];

// Redirects implemented as a Vite plugin (instead of resolve.alias entries)
// so we can match by suffix against the *resolved* path; resolve.alias is
// applied to the source import string and would need a regex per entry.
function liteparseRedirectsPlugin(): Plugin {
  return {
    name: "liteparse-redirects",
    enforce: "pre",
    async resolveId(source, importer, options) {
      // Vite will call us with `source` like `./pdfium-renderer.js` when one
      // of liteparse's internal files imports its sibling. Resolve through
      // the default pipeline first so we can pattern-match against the
      // fully-resolved (absolute) path. We forward `options` so the commonjs
      // plugin's hint propagation works correctly (otherwise Rollup logs a
      // warning about lost `this.resolve` options).
      const resolved = await this.resolve(source, importer, {
        ...options,
        skipSelf: true,
      });
      if (!resolved) return null;
      const id = resolved.id.replace(/\\/g, "/");
      // Only redirect inside @llamaindex/liteparse. Anything else (e.g. a
      // future tool that happens to have a /processing/gridVisualizer.js
      // file) is left alone.
      if (!id.includes("/@llamaindex/liteparse/")) return null;
      for (const { suffix, target } of FILE_REDIRECTS) {
        if (id.endsWith(suffix)) return target;
      }
      return null;
    },
  };
}

// The vendored pdf.mjs / pdf.worker.mjs files live inside @llamaindex/
// liteparse but are hidden from `import` by the package's `exports` map.
// We expose them through two virtual specifiers:
//
//   virtual:liteparse-pdfjs           -> the pdf.mjs ESM module
//   virtual:liteparse-pdfjs-worker    -> a module exporting the worker
//                                        source as a string (default)
//
// Using a plugin (rather than `resolve.alias` with `?raw`) keeps the query
// modifier out of the resolution path -- aliases don't preserve query
// strings cleanly across Vite 7, and we want the worker source materialised
// as a `JSON.stringify`'d literal so it's bundled inline and Blob-URL'able
// at runtime without any extra Vite transforms.
function liteparsePdfjsPlugin(
  vendoredPdfJs: string,
  vendoredPdfWorker: string,
): Plugin {
  const PDFJS_ID = "virtual:liteparse-pdfjs";
  const WORKER_ID = "virtual:liteparse-pdfjs-worker";
  const WORKER_RESOLVED = "\0virtual:liteparse-pdfjs-worker";
  return {
    name: "liteparse-pdfjs",
    enforce: "pre",
    resolveId(source) {
      if (source === PDFJS_ID) return vendoredPdfJs;
      if (source === WORKER_ID) return WORKER_RESOLVED;
      return null;
    },
    load(id) {
      if (id === WORKER_RESOLVED) {
        const src = readFileSync(vendoredPdfWorker, "utf-8");
        return `export default ${JSON.stringify(src)};`;
      }
      return null;
    },
  };
}

export default defineConfig({
  resolve: {
    alias: [
      // Bare-module aliases. `node:` specifiers match exactly; bare names
      // (e.g. `file-type`) match the package root, not deep imports.
      { find: /^node:fs$/, replacement: stub("empty.ts") },
      { find: /^node:fs\/promises$/, replacement: stub("empty.ts") },
      { find: /^node:child_process$/, replacement: stub("empty.ts") },
      { find: /^node:os$/, replacement: stub("empty.ts") },
      { find: /^node:stream$/, replacement: stub("empty.ts") },
      { find: /^node:stream\/promises$/, replacement: stub("empty.ts") },
      { find: /^node:tty$/, replacement: stub("empty.ts") },
      { find: /^node:crypto$/, replacement: stub("empty.ts") },
      { find: /^node:path$/, replacement: stub("node-path.ts") },
      { find: /^node:url$/, replacement: stub("node-url.ts") },
      { find: /^file-type$/, replacement: stub("file-type.ts") },
      // (pdf.mjs / pdf.worker.mjs are exposed via the `liteparsePdfjsPlugin`
      // below, not via alias -- see its comment for why.)
    ],
  },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    minify: "esbuild",
    sourcemap: false,
    chunkSizeWarningLimit: 8 * 1024,
    // Library mode produces a single self-contained ESM module at
    // `dist/liteparse.js`. Library mode (vs the default app build) is what
    // keeps Rollup from tree-shaking our re-exports as "unused" -- the
    // app-mode default assumes the bundle is consumed as a top-level entry
    // and aggressively strips any export not reachable from a side effect.
    lib: {
      entry: resolve(__dirname, "src/entry.ts"),
      formats: ["es"],
      fileName: () => "liteparse.js",
    },
    rollupOptions: {
      // Force-bundle everything: in library mode Vite externalises npm
      // deps by default; we override so the produced ESM is fully
      // self-contained (no runtime `import "@llamaindex/liteparse"` lookup
      // on the consumer page).
      external: [],
      output: {
        // `inlineDynamicImports: true` collapses any code-split chunks
        // (notably the top-level await in liteparse's pdfjs.js) into a
        // single output, so the deploy inliner has exactly one file to
        // splice into the HTML responder body.
        inlineDynamicImports: true,
        exports: "named",
      },
    },
  },
  plugins: [
    liteparseRedirectsPlugin(),
    liteparsePdfjsPlugin(vendoredPdfJs, vendoredPdfWorker),
  ],
});
