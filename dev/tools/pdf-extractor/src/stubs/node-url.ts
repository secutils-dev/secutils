// Minimal `node:url` shim for the browser bundle. Copied from
// simonw/liteparse@web.
export function fileURLToPath(u: string | URL): string {
  return typeof u === "string" ? u : u.href;
}
export function pathToFileURL(p: string): URL {
  return new URL(p, "file://");
}
export const URL = globalThis.URL;
