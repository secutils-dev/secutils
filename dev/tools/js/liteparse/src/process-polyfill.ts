// Browser-side polyfill for the Node `process` global. Liteparse and the
// vendored pdf.mjs reach for `process.stderr.write`, `process.env.X`, and
// `process.versions` in a handful of code paths. Installing a no-op shim
// up-front means every one of those is safe without rewriting liteparse.
//
// Critical: this module must be imported BEFORE any liteparse / pdf.mjs
// module so the shim is in place by the time their top-level code runs.
// The bundle's entry.ts does exactly that -- this module is imported on
// the first line, before `@llamaindex/liteparse`.
const g = globalThis as { process?: Record<string, unknown> };
if (typeof g.process === "undefined") {
  const noop = () => true;
  const stream = { write: noop, isTTY: false } as const;
  g.process = {
    env: {} as Record<string, string | undefined>,
    stderr: stream,
    stdout: stream,
    versions: {} as Record<string, string>,
    platform: "browser",
    cwd: () => "/",
    nextTick: (cb: (...args: unknown[]) => void, ...args: unknown[]) =>
      queueMicrotask(() => cb(...args)),
  };
} else {
  // Some sandboxes (CodeSandbox, Cursor's webview) pre-define a partial
  // `process`. Fill in just the bits liteparse expects so we don't clobber
  // anything else the host might be using.
  const p = g.process as Record<string, unknown>;
  if (!p.env) p.env = {};
  if (!p.stderr) p.stderr = { write: () => true, isTTY: false };
  if (!p.stdout) p.stdout = { write: () => true, isTTY: false };
}
