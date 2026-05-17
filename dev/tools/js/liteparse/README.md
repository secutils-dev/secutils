# `@secutils-dev/liteparse-browser`

Browser-compatible Vite build of [`@llamaindex/liteparse`](https://github.com/run-llama/liteparse).
Produces a single self-contained ESM module at `dist/liteparse.js` that is
inlined into `dev/tools/pdf-extractor.html` at deploy time by
`dev/tools/deploy.ts`'s `data-su-bundle="liteparse"` mechanism.

Not published. Internal to this repo. See
[`dev/tools/AGENTS.md`](../../AGENTS.md) -> "Embedded JS bundles" for the
inliner contract.

## Build

```bash
npm ci
npm run build
# => dist/liteparse.js
```

`make tools-bundles` from the repo root builds this (and any future bundles)
in one go. `make deploy-tools` builds it on demand if `dist/liteparse.js` is
missing or older than the sources here.

## Pinned upstream versions

| dep                       | version     | notes                                           |
|---------------------------|-------------|-------------------------------------------------|
| `@llamaindex/liteparse`   | `1.5.3`     | exact pin -- we patch its internals via stubs   |
| `tesseract.js`            | `^6.0.1`    | OCR engine; lazy-fetches worker + lang from CDN |
| `vite`                    | `^7.1.13`   | bundler                                         |
| (vendored) `pdfjs-dist`   | `5.6.205`   | shipped inside `@llamaindex/liteparse` itself   |

The `pdfjs-dist` build is **not** a direct dep -- we alias
`virtual:liteparse-pdfjs` / `virtual:liteparse-pdfjs-worker` to the copy
inside `node_modules/@llamaindex/liteparse/dist/src/vendor/pdfjs/`. Same
version liteparse was built against, no second 4 MB checkout.

## Stub strategy (hybrid)

The skeleton (which modules to redirect, name conventions) comes from
upstream's
[`run-llama/liteparse/scripts/browser-compat/`](https://github.com/run-llama/liteparse/tree/main/scripts/browser-compat).
The functional pieces (the PDF.js renderer + importer + safari polyfill)
come from
[`simonw/liteparse@web`](https://github.com/simonw/liteparse/tree/web/web)
which is a working browser fork.

| stub                                | source                | replaces upstream import                          |
|-------------------------------------|-----------------------|----------------------------------------------------|
| `src/stubs/pdfium-renderer.ts`      | simonw (functional)   | `engines/pdf/pdfium-renderer.js`                   |
| `src/stubs/pdfjsImporter.ts`        | custom (Simon-shaped) | `engines/pdf/pdfjsImporter.js`                     |
| `src/stubs/http-simple.ts`          | run-llama (throw-only)| `engines/ocr/http-simple.js`                       |
| `src/stubs/convertToPdf.ts`         | run-llama             | `conversion/convertToPdf.js`                       |
| `src/stubs/gridVisualizer.ts`       | run-llama             | `processing/gridVisualizer.js`                     |
| `src/stubs/gridDebugLogger.ts`      | run-llama             | `processing/gridDebugLogger.js`                    |
| `src/stubs/file-type.ts`            | run-llama             | `file-type` npm pkg                                |
| `src/stubs/empty.ts`                | simonw (defensive)    | `node:fs`, `node:child_process`, `node:os`, ...   |
| `src/stubs/node-path.ts`            | simonw                | `node:path`                                        |
| `src/stubs/node-url.ts`             | simonw                | `node:url`                                         |

All redirects are configured in `vite.config.ts`:
- bare-module aliases (`node:*`, `file-type`, the two `virtual:liteparse-pdfjs*`
  specifiers) via `resolve.alias`,
- liteparse-internal file redirects via a small `resolveId` plugin that
  matches against the resolved absolute path and only kicks in inside
  `node_modules/@llamaindex/liteparse/`.

## Runtime caveats

1. **OCR fetches from CDN.** `tesseract.js` lazy-loads its worker and
   language data (e.g. `eng.traineddata`, ~10 MB) from `unpkg.com` and
   `tessdata.projectnaptha.com` on first OCR invocation. The PDF Extractor
   tool's privacy dialog discloses this and gates OCR behind an explicit
   "Enable OCR" toggle.
2. **No cmaps / standard fonts.** Our `importPdfJs()` returns `dir: ""`, so
   pdf.js doesn't load cmap data. Latin-script PDFs render fine; CJK and
   some specialised PDFs fall back to substitute glyphs. Acceptable for v1.
3. **File paths are not supported.** `LiteParse.parse(path)` overload throws;
   the HTML tool always passes a `Uint8Array` (`File.arrayBuffer()`).
4. **Office docs (DOCX, XLSX, ...) are not supported.** Upstream relies on
   libreoffice; the conversion stub throws on use. The HTML tool rejects
   non-PDF files at the dropzone.
5. **Workers run on the main thread (pdf.js).** Actually no -- pdf.js gets a
   real worker via the Blob URL we set in `pdfjsImporter.ts`. tesseract.js
   also runs in its own worker.

## Re-syncing against a newer `@llamaindex/liteparse`

1. Bump the pin in `package.json` (exact version, not a range).
2. `npm install`.
3. Diff the suffixes our `resolveId` plugin keys off
   (`/engines/pdf/pdfium-renderer.js`, `/engines/pdf/pdfjsImporter.js`,
   `/engines/ocr/http-simple.js`, `/conversion/convertToPdf.js`,
   `/processing/gridVisualizer.js`, `/processing/gridDebugLogger.js`)
   against the new tarball: `tar -tzf $(npm pack --dry-run --json | jq -r .[0].filename)`.
   If any of those move, update `FILE_REDIRECTS` in `vite.config.ts`.
4. Check the vendored pdf.mjs version (`grep -m1 pdfjsVersion node_modules/@llamaindex/liteparse/dist/src/vendor/pdfjs/pdf.mjs`)
   and update the "Pinned upstream versions" table here.
5. `npm run build` and look for new "[liteparse-browser] accessed stubbed
   Node module property: X" errors in the browser console when running the
   tool end-to-end. Each one means a new Node-only import path crept in and
   needs a stub.
6. Rebuild + redeploy: `make tools-bundles && make deploy-tools ARGS="pdf-extractor"`.
