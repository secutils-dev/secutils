import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

const source = readFileSync(new URL('../src/share/share.ts', import.meta.url), 'utf8');
const start = source.indexOf('export async function copyNodeLink');
const body = source.slice(start, source.indexOf('// Short link when possible', start)).replace(/^export /, '').trim();
const javascript = transformSync(body, { loader: 'ts' }).code;

for (const kind of ['full', 'short']) {
    test(`${kind} clipboard writes start synchronously while document preparation waits for persistence`, async () => {
        let release!: () => void, invoked = false, captured: Promise<string> | null = null;
        let text = 'Old document';
        const updated = '<!-- su-table {"v":1,"cols":[0.7,0.3]} -->\n| A | B |\n| --- | --- |';
        const ready = new Promise<void>((resolve) => { release = resolve; }).then(() => { text = updated; });
        const copy = runInNewContext(`${javascript}\ncopyNodeLink`, {
            app: { state: { currentId: 'doc' } },
            viewer: { flush: () => ready, toast: () => {} },
            bundleFor: async () => ({ docs: [{ text }] }),
            fullLink: async (bundle: { docs: { text: string }[] }) => `full:${bundle.docs[0].text}`,
            shortLink: async (_: string, bundle: { docs: { text: string }[] }) => ({ url: `short:${bundle.docs[0].text}` }),
            copyRich: async (url: Promise<string>) => { invoked = true; captured = url; await url; },
            ttlLabel: () => '7 days', shortTtl: 604800,
        });
        const pending = copy('doc', kind);
        assert.equal(invoked, true, 'The clipboard write must begin before the first await');
        release();
        await pending;
        assert.equal(await captured, `${kind}:${updated}`);
    });
}
