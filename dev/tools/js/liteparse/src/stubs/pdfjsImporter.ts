// Browser replacement for `@llamaindex/liteparse`'s pdfjsImporter. Loads
// pdf.mjs from the vendored copy that already ships inside the npm package
// (aliased to `virtual:liteparse-pdfjs` in vite.config.ts) and configures the
// worker as a Blob URL constructed from the worker source bundled inline
// via Vite's `?raw` query.
//
// Why a Blob URL for the worker?
//   - The whole bundle is later inlined into the tool HTML responder by
//     dev/tools/deploy.ts and served from a `Blob:` URL on the consumer
//     page. Asking pdf.js to load a separate worker file via `import.meta.url`
//     wouldn't work (no module URL in a Blob-imported module), and a CDN
//     workerSrc would be the only network-touching part of the bundle.
//     `?raw` -> Blob URL keeps the whole pipeline same-origin and offline.
//   - The fallback `disableWorker: true` (a.k.a. "fake worker") is a non-
//     option because pdf.js 4+ removed it: the worker IS the parser.

// Safari < 17 / older iOS WebKit ship ReadableStream but not its
// `[Symbol.asyncIterator]`. pdf.js uses `for await (const v of stream)`
// internally while streaming text content; without this polyfill the first
// parse throws "undefined is not a function". Install BEFORE importing
// pdf.mjs so any early code paths see it.
if (
  typeof ReadableStream !== "undefined" &&
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  !(Symbol.asyncIterator in (ReadableStream.prototype as any))
) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (ReadableStream.prototype as any)[Symbol.asyncIterator] =
    async function* iterator() {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const reader = (this as any).getReader();
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) return;
          yield value;
        }
      } finally {
        reader.releaseLock();
      }
    };
}

// @ts-expect-error vendored ESM build has no types
import * as pdfjs from "virtual:liteparse-pdfjs";
// @ts-expect-error virtual module produced by the liteparsePdfjsPlugin; exports the
// pdf.worker.mjs source as a JSON-stringified default. We turn it into a Blob URL
// below so pdf.js can spawn a Worker without any extra HTTP fetches.
import pdfWorkerSource from "virtual:liteparse-pdfjs-worker";

const workerBlobUrl = URL.createObjectURL(
  new Blob([pdfWorkerSource as string], { type: "text/javascript" }),
);
(pdfjs as { GlobalWorkerOptions: { workerSrc: string } }).GlobalWorkerOptions.workerSrc =
  workerBlobUrl;

export async function importPdfJs() {
  return {
    fn: (pdfjs as { getDocument: (opts: unknown) => unknown }).getDocument,
    // Base URL for cmaps / standard_fonts / wasm. Empty string disables them
    // -- pdf.js falls back to substitute glyphs for CJK / specialised PDFs.
    // Acceptable for the PDF Extractor tool's v1 scope (Latin-script PDFs);
    // a future enhancement can vend the cmap directory off the responder
    // host if there is demand.
    dir: "",
  };
}
