// One assembly function for local preview, deployment, and sandbox verification.
// The compatibility shims stay inside the dispatched branch, not on the sandbox global.
export function inlineMarsSync(script: string, compiled: string): string {
    if (!script.includes('// @su:mars-sync')) return script;
    return script.replace('(() => {', '(async () => {').replaceAll('secutils.kv', 'marsKv').replace('// @su:mars-sync', () => `
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
    }`);
}
