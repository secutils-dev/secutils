// One assembly function for local preview, deployment, and sandbox verification.
// The compatibility shims stay inside the sync branch, not on the sandbox global.
export function inlineMarsSync(script: string, compiled: string): string {
    if (!script.includes('// @su:mars-sync')) return script;
    return script.replace('// @su:mars-sync', () => `
    if ((context.path || '').startsWith('/sync/')) {
        const TextEncoder = class { encode(value) { return Deno.core.encode(value); } };
        const TextDecoder = class { decode(value) { return Deno.core.decode(value); } };
        ${compiled}
        return MARS_SYNC.handle(context, secutils);
    }`);
}
