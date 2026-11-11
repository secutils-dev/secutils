// Browser stub for liteparse's conversion pipeline. Upstream shells out to
// libreoffice / soffice for DOCX / XLSX / HTML -> PDF conversion. The
// browser bundle only supports PDF input; non-PDF bytes get rejected at the
// UI layer. Lifted from run-llama/liteparse's browser-compat stubs (with a
// matching surface for the named exports the parser imports).
export const officeExtensions: string[] = [];
export const spreadsheetExtensions: string[] = [];
export const imageExtensions: string[] = [];
export const htmlExtensions: string[] = [];
export async function convertToPdf(): Promise<never> {
  throw new Error("File conversion is not supported in browser environments.");
}
export async function convertBufferToPdf(): Promise<never> {
  throw new Error("File conversion is not supported in browser environments.");
}
export async function cleanupConversionFiles(): Promise<void> {}
export async function guessExtensionFromBuffer(
  data: Uint8Array,
): Promise<string | null> {
  if (data[0] === 0x25 && data[1] === 0x50 && data[2] === 0x44 && data[3] === 0x46) {
    return ".pdf";
  }
  return null;
}
