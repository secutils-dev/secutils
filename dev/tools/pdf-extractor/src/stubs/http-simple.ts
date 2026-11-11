// Browser stub -- the remote HTTP OCR engine is not exposed by the PDF
// Extractor tool. Tesseract.js is the in-browser OCR path. Lifted from
// run-llama/liteparse's browser-compat stubs.
export class HttpOcrEngine {
  name = "http-ocr";
  constructor() {
    throw new Error(
      "HTTP OCR engine is not available in browser environments.",
    );
  }
  async recognize(): Promise<never> {
    throw new Error("Not available in browser.");
  }
  async recognizeBatch(): Promise<never> {
    throw new Error("Not available in browser.");
  }
}
