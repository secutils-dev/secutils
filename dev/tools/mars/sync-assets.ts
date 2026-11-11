// One assembly function for local preview, deployment, and sandbox verification.
// The compatibility shims stay inside the dispatched branch, not on the sandbox global.

/**
 * Returns the snapshot responder script with the compiled Sync responder inlined at its
 * `// @su:mars-sync` marker. A script without the marker returns unchanged.
 *
 * The result differs from the input in three ways:
 * - The outer function becomes `async`, because the inlined branch waits for the test scope.
 * - Each `secutils.kv` of the snapshot code becomes `marsKv`, so a test scope can give the
 *   short-link store an isolated key-value store.
 * - Requests under `/sync/` go to `MARS_SYNC.handle` and never reach the snapshot code.
 *
 * @param script The `@su:responder-script` text of `index.html`.
 * @param compiled The content of `dist/mars-sync-responder.js`, which defines `MARS_SYNC`.
 */
export function inlineMarsSync(script: string, compiled: string): string {
    if (!script.includes('// @su:mars-sync')) return script;
    // The marker replacement is a function, so a `$` sequence in the compiled code is not read as
    // a pattern. It runs after `replaceAll`, so the `secutils.kv` of the inlined branch stays.
    return script
        .replace('(() => {', '(async () => {')
        .replaceAll('secutils.kv', 'marsKv')
        .replace(
            '// @su:mars-sync',
            () => `
    let marsKv = secutils.kv;
    if ((context.path || '').startsWith('/sync/') || context.headers?.['x-mars-test-scope'] || context.headers?.['x-mars-test-token']) {
        const TextEncoder = class { encode(value) { return Deno.core.encode(value); } };
        const TextDecoder = class { decode(value) { return Deno.core.decode(value); } };
        ${compiled}
        const prepared = await MARS_SYNC.prepareTestScope(context, secutils);
        if (prepared.response) return prepared.response;
        const runtime = prepared.runtime || secutils;
        marsKv = runtime.kv;
        if ((context.path || '').startsWith('/sync/')) return MARS_SYNC.handle(context, runtime);
    }`,
        );
}
