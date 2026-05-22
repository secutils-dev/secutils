// Throw-on-access stub for Node built-ins (`fs`, `url`, `path`) that the
// upstream `micro-ml/dist/index.js` performs dynamic `await import(...)`s on
// inside its `typeof process == "node"` guard. The guard is `false` in the
// browser, so the imports never execute -- but Vite still resolves them
// statically and emits a `Module "fs" has been externalized for browser
// compatibility` warning per import.
//
// Aliasing the three bare specifiers to this module via `resolve.alias` in
// `vite.config.ts` silences the warnings and (defensively) surfaces a clear
// error if the runtime guard ever does fall through (e.g. someone runs the
// bundle under a Node REPL that polyfills `process` with `node:` versions).
//
// We export every named binding the upstream destructures from the three
// modules: `fs.readFileSync`, `fs.existsSync`, `url.fileURLToPath`,
// `path.dirname`, `path.join`. Adding new ones is cheap if a future
// `micro-ml` release destructures additional helpers from these modules.

const stub =
  (name: string) =>
  (..._args: unknown[]): never => {
    throw new Error(
      `[@secutils-dev/micro-ml-browser] Node builtin '${name}' is not available in the browser; ` +
        "this code path should be unreachable behind upstream's `typeof process == \"node\"` guard.",
    );
  };

export const readFileSync = stub("readFileSync");
export const existsSync = stub("existsSync");
export const fileURLToPath = stub("fileURLToPath");
export const dirname = stub("dirname");
export const join = stub("join");

export default {} as Record<string, never>;
