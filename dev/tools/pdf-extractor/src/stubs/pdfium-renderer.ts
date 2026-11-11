// Browser replacement for `@llamaindex/liteparse`'s PDFium renderer. Uses
// PDF.js's canvas renderer for both OCR-input rendering and screenshots
// (screenshots are wired up here for completeness; the PDF Extractor tool
// does not expose them in v1, but the LiteParse public API may still call
// `renderPageToBuffer` for the "render-page-then-OCR-it" path on
// text-sparse pages). No PDFium, no sharp, pure PDF.js + OffscreenCanvas.
//
// Sourced from simonw/liteparse@web and lightly tightened (typed Promises
// preserved, identical public surface to upstream pdfium-renderer.js so
// liteparse's parser.js can swap renderers without code changes).

import { importPdfJs } from "./pdfjsImporter.js";

interface PdfJsDocument {
  numPages: number;
  getPage(n: number): Promise<PdfJsPage>;
  destroy(): Promise<void>;
}
interface PdfJsPage {
  getViewport(opts: { scale: number }): { width: number; height: number };
  render(opts: {
    canvasContext: unknown;
    viewport: { width: number; height: number };
  }): { promise: Promise<void> };
  cleanup(): Promise<void>;
}

export class PdfiumRenderer {
  private doc: PdfJsDocument | null = null;

  async init(): Promise<void> {
    // no-op; lazy-init happens in loadDocument
  }

  async loadDocument(pdfInput: string | Uint8Array): Promise<void> {
    if (typeof pdfInput === "string") {
      throw new Error("File paths are not supported in the browser renderer.");
    }
    this.closeDocument();
    const { fn: getDocument } = await importPdfJs();
    // Copy the bytes — pdf.js transfers the ArrayBuffer to its worker,
    // which would detach the caller's view.
    const data = new Uint8Array(pdfInput);
    const loadingTask = (
      getDocument as (opts: { data: Uint8Array }) => {
        promise: Promise<PdfJsDocument>;
      }
    )({ data });
    this.doc = await loadingTask.promise;
  }

  closeDocument(): void {
    if (this.doc) {
      this.doc.destroy().catch(() => {});
      this.doc = null;
    }
  }

  // Public accessor so callers (e.g. the PDF Extractor's Screenshots tab)
  // can drive a render loop without poking at the private `doc` field --
  // which is fine in pure TS but fragile across bundler property-mangling
  // settings.
  get numPages(): number {
    return this.doc?.numPages ?? 0;
  }

  private async renderCanvas(pageNumber: number, dpi: number) {
    if (!this.doc) throw new Error("Renderer has no document loaded.");
    const page = await this.doc.getPage(pageNumber);
    const scale = dpi / 72;
    const viewport = page.getViewport({ scale });
    const canvas = new OffscreenCanvas(
      Math.ceil(viewport.width),
      Math.ceil(viewport.height),
    );
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("Failed to get 2D context");
    // White background — PDF.js renders transparent by default.
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    await page.render({ canvasContext: ctx, viewport }).promise;
    try {
      const maybe = page.cleanup();
      if (maybe && typeof (maybe as Promise<void>).then === "function") {
        await (maybe as Promise<void>);
      }
    } catch {
      // best-effort cleanup
    }
    return { canvas, ctx, width: canvas.width, height: canvas.height };
  }

  async renderPageToBuffer(
    _pdfInput: string | Uint8Array,
    pageNumber: number,
    dpi: number = 150,
  ): Promise<Uint8Array> {
    const { canvas } = await this.renderCanvas(pageNumber, dpi);
    const blob = await canvas.convertToBlob({ type: "image/png" });
    return new Uint8Array(await blob.arrayBuffer());
  }

  async renderPageToImageData(
    pageNumber: number,
    dpi: number,
  ): Promise<ImageData> {
    const { ctx, width, height } = await this.renderCanvas(pageNumber, dpi);
    return ctx.getImageData(0, 0, width, height);
  }

  async extractImageBounds(): Promise<
    Array<{ x: number; y: number; width: number; height: number }>
  > {
    // PDF.js doesn't expose embedded-image bounds cleanly. For v1 we return
    // nothing — the parser falls back to full-page OCR on text-sparse pages,
    // which is the common real-world case.
    return [];
  }

  async close(): Promise<void> {
    this.closeDocument();
  }
}
