// Browser stub for `file-type`. Liteparse only ever calls
// `fileTypeFromBuffer(bytes)` to decide whether an input is a PDF (it always
// is, in the browser) or needs format conversion (we don't support that in
// the browser anyway). The upstream package depends on `peek-readable`, which
// expects Node Readable streams -- entirely the wrong shape for our bundle.
// Lifted from run-llama/liteparse's browser-compat stubs.
export async function fileTypeFromBuffer(buf: Uint8Array) {
  if (buf[0] === 0x25 && buf[1] === 0x50 && buf[2] === 0x44 && buf[3] === 0x46) {
    return { ext: "pdf", mime: "application/pdf" };
  }
  return undefined;
}
export async function fileTypeFromFile() {
  return undefined;
}
