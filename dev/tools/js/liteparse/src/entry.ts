// Browser entry point for the liteparse bundle. Re-exports the upstream
// LiteParse class and the helper utility we use from the consuming HTML
// tool. Vite's resolveId hook (see vite.config.ts) intercepts liteparse's
// Node-only internal imports and redirects them to the stubs in src/stubs/.
//
// At runtime in the browser:
//   - PDF parsing goes through the vendored PDF.js (no PDFium, no sharp).
//   - OCR (when enabled) goes through tesseract.js, which lazy-fetches its
//     worker and language data from the unpkg CDN. The consuming tool MUST
//     disclose this in its privacy dialog before enabling OCR.
//   - HTML tool input is `Uint8Array` (from a `File.arrayBuffer()` read).
//     `LiteParse.parse(input)` does the heavy lifting.

// IMPORTANT: this side-effect import MUST stay first. It installs the
// browser-side `process` polyfill before any liteparse module touches
// `process.stderr.write` / `process.env.X` / `process.versions` at top-level
// load time. ES module evaluation order is topological by import order, so
// putting this above the `@llamaindex/liteparse` import is load-bearing.
import "./process-polyfill.js";

export { LiteParse } from "@llamaindex/liteparse";
export type {
  LiteParseConfig,
  LiteParseInput,
  ParseResult,
  ScreenshotResult,
} from "@llamaindex/liteparse";

// Re-export the PDF.js-backed renderer so the consuming HTML tool can take
// per-page screenshots without round-tripping through LiteParse.parse() (which
// is geared toward text/OCR extraction). The renderer wraps PDF.js's canvas
// renderer; see src/stubs/pdfium-renderer.ts.
export { PdfiumRenderer } from "./stubs/pdfium-renderer.js";

import { importPdfJs } from "./stubs/pdfjsImporter.js";

/**
 * A hyperlink annotation extracted from a PDF, in liteparse coordinate space
 * (top-left origin, PDF points). The `page` field is 1-indexed to match
 * liteparse's `ParseResultJson.pages[*].page`.
 */
export interface PdfLink {
  page: number;
  x: number;
  y: number;
  width: number;
  height: number;
  url: string;
}

interface PdfJsAnnotation {
  subtype?: string;
  url?: string;
  unsafeUrl?: string;
  rect?: [number, number, number, number];
}

interface PdfJsPageForLinks {
  view: [number, number, number, number];
  getAnnotations(): Promise<PdfJsAnnotation[]>;
  cleanup?: () => Promise<void>;
}

interface PdfJsDocForLinks {
  numPages: number;
  getPage(n: number): Promise<PdfJsPageForLinks>;
  destroy(): Promise<void>;
}

/**
 * Extract all hyperlink annotations from a PDF. The PDF Extractor's Markdown
 * tab uses this for best-effort `[text](url)` reconstruction by intersecting
 * each `JsonTextItem`'s bbox with the returned link rectangles.
 *
 * PDF.js reports annotation rectangles in PDF user space (bottom-left origin).
 * liteparse's `JsonTextItem.x`/`.y` are top-left origin in PDF points (the
 * viewer convention). We convert here so callers can compare coordinates
 * directly without knowing about origin flips.
 */
export async function getPdfLinks(bytes: Uint8Array): Promise<PdfLink[]> {
  const { fn: getDocument } = await importPdfJs();
  const data = new Uint8Array(bytes);
  const loadingTask = (
    getDocument as (opts: { data: Uint8Array }) => {
      promise: Promise<PdfJsDocForLinks>;
    }
  )({ data });
  const doc = await loadingTask.promise;
  const out: PdfLink[] = [];
  try {
    for (let p = 1; p <= doc.numPages; p++) {
      const page = await doc.getPage(p);
      const annotations = await page.getAnnotations();
      const pageHeight = page.view[3] - page.view[1];
      for (const ann of annotations) {
        if (ann.subtype !== "Link") continue;
        const url = ann.url ?? ann.unsafeUrl;
        if (!url || !ann.rect) continue;
        const [x1, y1, x2, y2] = ann.rect;
        const lx = Math.min(x1, x2);
        const rx = Math.max(x1, x2);
        const ly = Math.min(y1, y2);
        const ry = Math.max(y1, y2);
        out.push({
          page: p,
          x: lx,
          y: pageHeight - ry,
          width: rx - lx,
          height: ry - ly,
          url,
        });
      }
      await page.cleanup?.();
    }
  } finally {
    await doc.destroy().catch(() => {});
  }
  return out;
}
