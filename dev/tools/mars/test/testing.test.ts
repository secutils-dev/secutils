import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import * as vm from 'node:vm';
import { build } from 'esbuild';
import { inlineMarsSync } from '../sync-assets.ts';
import { prepareTestScope } from '../src/sync/testing.ts';
import { decode, encode } from '../src/sync/protocol.ts';
import type { Context, Kv } from '../src/sync/responder.ts';

const operator = 'operator-secret';
const hash = (text: string) => createHash('sha256').update(text).digest('hex');
const token = Buffer.alloc(32, 7).toString('base64url');
class MemoryKv implements Kv {
    values = new Map<string, { value: string; expiresAt: number | null }>();
    operations = 0;
    afterSet: ((key: string) => Promise<void>) | null = null;
    count() { assert.ok(++this.operations <= 200); }
    async get(key: string) { this.count(); return this.values.get(key)?.value ?? null; }
    async getEntry(key: string) { this.count(); return this.values.get(key) ?? { value: null, expiresAt: null }; }
    async set(key: string, value: string, options: { ifAbsent?: boolean; ttlSec?: number } = {}) {
        this.count();
        assert.equal(Buffer.from(value, 'base64url').toString('base64url'), value, 'Match native KV canonical base64 validation');
        if (!options.ifAbsent || !this.values.has(key)) this.values.set(key, { value, expiresAt: options.ttlSec ? Math.floor(Date.now() / 1000) + options.ttlSec : null });
        await this.afterSet?.(key);
    }
    async delete(key: string) { this.count(); return this.values.delete(key); }
    async list({ prefix, after = '', limit, valuesIncluded = true }: Parameters<Kv['list']>[0]) {
        this.count();
        const entries = [...this.values].filter(([key]) => key.startsWith(prefix) && key > after).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
            .slice(0, limit).map(([key, item]) => ({ key, ...(valuesIncluded ? { value: item.value } : {}) }));
        return { entries, cursor: entries.at(-1)?.key ?? null };
    }
}
async function fixture() {
    const kv = new MemoryKv(), scope = crypto.randomUUID(), run = crypto.randomUUID(), testId = hash('test');
    const runtime = { kv, crypto: { sha256: (value: string) => hash(Buffer.from(value, 'base64url').toString()) } };
    const request = async (body: Record<string, unknown>, secret = operator) => {
        kv.operations = 0;
        const result = await prepareTestScope({ path: '/sync/testing', method: 'POST', body: new TextEncoder().encode(JSON.stringify({ scope, run, test: testId, ...body })),
            headers: { authorization: `Bearer ${secret}` }, secrets: { MARS_STATS_API_KEY: operator } }, runtime);
        return { status: result.response!.statusCode, body: JSON.parse(result.response!.body) };
    };
    const open = () => request({ operation: 'open', title: 'Synthetic test', tokenHash: hash(token) });
    const context: Context = { path: '/sync/index', method: 'POST', body: new Uint8Array(), headers: { 'x-mars-test-scope': scope, 'x-mars-test-token': token } };
    const scoped = async () => { kv.operations = 0; return (await prepareTestScope(context, runtime)).runtime!.kv; };
    return { kv, scope, run, testId, runtime, context, request, open, scoped };
}

test('test scopes require operator authentication and bind cleanup to exact run/test metadata', async () => {
    const f = await fixture();
    assert.equal((await f.request({ operation: 'open', title: 'Synthetic test', tokenHash: hash(token) }, 'wrong')).status, 403);
    assert.equal(f.kv.values.size, 0);
    assert.equal((await f.open()).status, 201);
    assert.equal((await f.open()).status, 200);
    assert.equal((await f.request({ operation: 'open', title: 'Other test', tokenHash: hash(token) })).status, 409);
    assert.equal((await f.request({ operation: 'close', run: crypto.randomUUID() })).status, 403);
    assert.equal((await f.request({ operation: 'close', test: hash('other') })).status, 403);
    const list = await f.request({ operation: 'list' });
    assert.equal(list.body.scopes[0].run, f.run);
    assert.equal(list.body.scopes[0].title, 'Synthetic test');
    assert.equal(JSON.stringify(list.body).includes(hash(token)), false);
    assert.equal((await f.request({ operation: 'list', after: 's/' })).status, 400);
});

test('test KV isolates snapshots, Sync ledgers, key scans, and cursors without changing permanent-entry semantics', async () => {
    const f = await fixture(); await f.open();
    f.kv.values.set('s/link', { value: encode(new TextEncoder().encode('legitimate')), expiresAt: 123 });
    const scoped = await f.scoped();
    await scoped.set('s/link', encode(new TextEncoder().encode('synthetic')));
    await scoped.set('s/link2', 'AA'); await scoped.set('sync/v1/budget/all/00000/00001', 'AA');
    assert.equal(new TextDecoder().decode(decode((await scoped.get('s/link'))!)), 'synthetic');
    assert.equal(new TextDecoder().decode(decode(f.kv.values.get('s/link')!.value)), 'legitimate');
    assert.equal((await scoped.getEntry('sync/v1/budget/all/00000/00001')).expiresAt, null);
    const page = await scoped.list({ prefix: 's/', limit: 1 });
    assert.deepEqual(page.entries.map(entry => entry.key), ['s/link']); assert.equal(page.cursor, 's/link');
    assert.deepEqual((await scoped.list({ prefix: 's/', after: page.cursor!, limit: 1 })).entries.map(entry => entry.key), ['s/link2']);
    assert.equal((await scoped.list({ prefix: '../', limit: 1 })).entries.length, 0);
});

test('test cleanup is bounded, repeatable, preserves other namespaces, and retains only expiring audit metadata', async () => {
    const f = await fixture(); await f.open();
    const prefix = `testing/v1/data/${f.scope}/`;
    for (let i = 0; i < 131; i++) f.kv.values.set(`${prefix}entry/${i}`, { value: 'AA', expiresAt: null });
    f.kv.values.set('s/legitimate', { value: 'AA', expiresAt: null });
    f.kv.values.set('testing/v1/data/another-scope/entry', { value: 'AA', expiresAt: null });
    let answer;
    do { answer = await f.request({ operation: 'close' }); assert.equal(answer.status, 200); } while (!answer.body.complete);
    assert.equal(answer.body.removed, 131);
    assert.equal([...f.kv.values.keys()].some(key => key.startsWith(prefix)), false);
    assert.ok(f.kv.values.has('s/legitimate')); assert.ok(f.kv.values.has('testing/v1/data/another-scope/entry'));
    assert.equal((await f.request({ operation: 'close' })).body.removed, 131);
    const record = f.kv.values.get(`testing/v1/scopes/${f.scope}`)!;
    assert.ok(record.expiresAt);
    f.kv.operations = 0;
    assert.equal((await prepareTestScope(f.context, f.runtime)).response?.statusCode, 410);
    assert.equal((await f.open()).status, 409);
});

test('missing, forged, and expired test credentials never fall back to user storage', async (t) => {
    const f = await fixture(); await f.open();
    for (const headers of [{ 'x-mars-test-scope': f.scope }, { 'x-mars-test-token': token },
        { 'x-mars-test-scope': f.scope, 'x-mars-test-token': Buffer.alloc(32, 8).toString('base64url') }]) {
        f.kv.operations = 0;
        const result = await prepareTestScope({ ...f.context, headers }, f.runtime);
        assert.equal(result.response?.statusCode, 403); assert.equal(result.runtime, undefined);
    }
    const later = Date.now() + 3601000; t.mock.method(Date, 'now', () => later);
    f.kv.operations = 0;
    assert.equal((await prepareTestScope(f.context, f.runtime)).response?.statusCode, 410);
    assert.equal((await f.request({ operation: 'close' })).status, 200);
});

test('a scoped write that completes after cleanup cannot recreate orphaned data', async () => {
    const f = await fixture(); await f.open(); const scoped = await f.scoped();
    f.kv.afterSet = async key => { if (key.includes('/data/')) { f.kv.afterSet = null; await f.request({ operation: 'close' }); } };
    await assert.rejects(scoped.set('s/late', 'AA'), /test-scope-ended/);
    assert.equal([...f.kv.values.keys()].some(key => key.includes('/data/')), false);
});

test('the deployment assembly isolates real snapshot responder requests and leaves ordinary short links intact', async () => {
    const f = await fixture(); await f.open();
    const compiled = await build({ entryPoints: ['src/sync/responder.ts'], bundle: true, minify: true, format: 'iife', globalName: 'MARS_SYNC', write: false });
    const source = readFileSync(new URL('../index.html', import.meta.url), 'utf8').match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)![1];
    const script = inlineMarsSync(source, compiled.outputFiles[0].text);
    const invoke = async (method: string, scoped: boolean, body = 'A'.repeat(43)) => {
        f.kv.operations = 0;
        const sandbox = vm.createContext({ context: { path: '/', method, query: { s: 'abcdefghijklmnop' }, body: new TextEncoder().encode(body),
            headers: { 'x-su-delete-token': 'A'.repeat(22), ...(scoped ? f.context.headers : {}) } }, secutils: f.runtime, Uint8Array,
            Deno: { core: { encode: (value: string) => new TextEncoder().encode(value), decode: (value: Uint8Array) => new TextDecoder().decode(value) } } });
        return await vm.runInContext(script, sandbox);
    };
    const ordinary = Buffer.alloc(32, 17).toString('base64url');
    assert.equal((await invoke('POST', false, ordinary)).statusCode, 201);
    assert.equal((await invoke('POST', true)).statusCode, 201);
    assert.equal(JSON.parse((await invoke('GET', true)).body).v, 'A'.repeat(43));
    await f.request({ operation: 'close' });
    assert.equal((await invoke('GET', true)).statusCode, 410);
    assert.equal(JSON.parse((await invoke('GET', false)).body).v, ordinary);
});
