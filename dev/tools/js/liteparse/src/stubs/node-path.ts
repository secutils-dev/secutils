// Minimal POSIX `path` shim for the browser bundle. Liteparse's grid /
// bbox code calls `path.basename` and `path.extname` on log labels, so an
// empty stub is not enough -- these have to do the right thing for
// POSIX-style strings. Copied from simonw/liteparse@web.
export function dirname(p: string): string {
  const i = p.lastIndexOf("/");
  return i < 0 ? "." : p.slice(0, i) || "/";
}
export function basename(p: string, ext?: string): string {
  const i = p.lastIndexOf("/");
  const base = i < 0 ? p : p.slice(i + 1);
  if (ext && base.endsWith(ext)) return base.slice(0, -ext.length);
  return base;
}
export function join(...parts: string[]): string {
  return parts.filter(Boolean).join("/").replace(/\/+/g, "/");
}
export function resolve(...parts: string[]): string {
  return join(...parts);
}
export function extname(p: string): string {
  const i = p.lastIndexOf(".");
  return i < 0 ? "" : p.slice(i);
}
export const sep = "/";
export default { dirname, basename, join, resolve, extname, sep };
