import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { handle, type Kv } from '../src/sync/responder.ts';
import { checkpointDigest, identity, keyBytes, randomSecret, sealCheckpoint, signGrant, tokenHash } from '../src/sync/crypto.ts';
import { decode, LIMITS, type Grant } from '../src/sync/protocol.ts';
import { inlineMarsSync } from '../../../mars-sync-assets.ts';

// The runner links the existing secutils::js_runtime::JsRuntime with its real
// 10 MiB heap and 10-second deadline. KV is stubbed; signatures and SHA-256 are real.
test('deployment-assembled sync executes at native heap and invocation limits', { skip: !process.env.MARS_NATIVE_RUNNER }, async () => {
    assert.ok(process.env.MARS_NATIVE_DIR, 'Set MARS_NATIVE_DIR to an approved temporary directory');
    const dir = mkdtempSync(join(process.env.MARS_NATIVE_DIR, 'mars-native-'));
    const source = readFileSync(new URL('../../../mars.html', import.meta.url), 'utf8').match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)![1];
    const assembled = inlineMarsSync(source, readFileSync(new URL('../dist/mars-sync-responder.js', import.meta.url), 'utf8'));
    const script = `(async () => {
        ${process.env.MARS_NATIVE_TRACE ? "Deno.core.print('Native base64: ' + typeof Uint8Array.prototype.toBase64 + '\\n', true);" : ''}
        // Native KV retains bytes outside V8. Model that, rather than retaining all
        // stored base64 strings in the isolate even when a list excludes values.
        const input = context.body;
        const values = new Map(context.seed.map(([key, item]) => [key, { value: input.subarray(item.start, item.end), expiresAt: item.expiresAt }])), crypto = secutils.crypto;
        context.body = input.subarray(0, context.requestBytes);
        delete context.seed;
        let operations = 0;
        const count = () => { if (++operations > 200) throw new Error('KV operation budget'); ${process.env.MARS_NATIVE_TRACE ? "Deno.core.print('KV operation ' + operations + '\\n', true);" : ''} };
        secutils = { crypto, kv: {
            get: async key => { count(); const item = values.get(key); return item ? Deno.core.decode(item.value) : null; },
            getEntry: async key => { count(); const item = values.get(key); return item ? { ...item, value: Deno.core.decode(item.value) } : { value: null, expiresAt: null }; },
            set: async (key, value, opts = {}) => {
                count(); if (Math.floor(value.length * 3 / 4) > 1048576) throw new Error('KV value cap');
                if (!opts.ifAbsent || !values.has(key)) values.set(key, { value: Deno.core.encode(value), expiresAt: opts.ttlSec ? Math.floor(Date.now() / 1000) + opts.ttlSec : null });
            },
            delete: async key => { count(); return values.delete(key); },
            list: async ({ prefix, after = '', limit, valuesIncluded = true }) => {
                count(); const entries = [...values].filter(([key]) => key.startsWith(prefix) && key > after).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).slice(0, Math.min(limit, 1000)).map(([key, value]) => ({ key, ...(valuesIncluded ? { value: Deno.core.decode(value.value) } : {}) }));
                return { entries, cursor: entries.at(-1)?.key ?? null };
            }
        } };
        const result = await (${assembled.trim().replace(/;$/, '')});
        return JSON.stringify({ result, operations });
    })()`;
    writeFileSync(join(dir, 'script.js'), script);
    const values = new Map<string, { value: string; expiresAt: number | null }>();
    const kv: Kv = {
        get: async (key) => values.get(key)?.value ?? null,
        getEntry: async (key) => values.get(key) ?? { value: null, expiresAt: null },
        set: async (key, value, options = {}) => { if (!options.ifAbsent || !values.has(key)) values.set(key, { value, expiresAt: options.ttlSec ? Math.floor(Date.now() / 1000) + options.ttlSec : null }); },
        delete: async (key) => values.delete(key),
        list: async ({ prefix, after = '', limit }) => { const entries = [...values].filter(([key]) => key.startsWith(prefix) && key > after).sort().slice(0, limit).map(([key, item]) => ({ key, value: item.value })); return { entries, cursor: entries.at(-1)?.key ?? null }; },
    };
    const who = await identity(), token = randomSecret(), document = crypto.randomUUID();
    const grant: Grant = { v: 1, collection: crypto.randomUUID(), member: crypto.randomUUID(), participant: crypto.randomUUID(), permission: 'owner', document: null,
        publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(token), expires: 0 };
    const owner = await signGrant(grant, who);
    const runtime = { kv, crypto: { sha256: (value: string) => createHash('sha256').update(Buffer.from(value, 'base64url')).digest('hex') } };
    for (const [action, extra] of [['create', { owner }], ['document', { document, kind: 'doc' }]] as const) {
        const result = await handle({ path: `/sync/${action}`, method: 'POST', headers: { authorization: `Bearer ${token}` },
            body: new TextEncoder().encode(JSON.stringify({ collection: grant.collection, member: grant.member, ...extra })) }, runtime);
        assert.equal(result?.statusCode, 201);
    }
    const run = (action: string, body: string, seed = [...values]) => {
        const request = Buffer.from(body), chunks = [request]; let offset = request.length;
        const entries = seed.map(([key, item]) => {
            const bytes = Buffer.from(item.value), start = offset; offset += bytes.length; chunks.push(bytes);
            return [key, { start, end: offset, expiresAt: item.expiresAt }];
        });
        writeFileSync(join(dir, 'context.json'), JSON.stringify({ path: `/sync/${action}`, method: action === 'limits' ? 'GET' : 'POST', body: null,
            headers: { authorization: `Bearer ${token}` }, seed: entries, requestBytes: request.length }));
        writeFileSync(join(dir, 'body.bin'), Buffer.concat(chunks));
        const result = spawnSync(process.env.MARS_NATIVE_RUNNER!, [join(dir, 'script.js'), join(dir, 'context.json'), join(dir, 'body.bin')], { encoding: 'utf8', maxBuffer: 12 * 1024 * 1024 });
        assert.equal(result.status, 0, `Native ${action}, ${body.length} request bytes: ${result.stderr}`);
        const output = JSON.parse(result.stdout);
        assert.equal(output.result.skipRequest, true);
        assert.ok(output.operations <= 200);
        return output;
    };
    assert.equal(run('limits', '').result.statusCode, 200);
    assert.equal(run('append', '{').result.statusCode, 400);
    let sequence = 0;
    for (const bytes of [512 * 1024, 740 * 1024, (LIMITS.updateBytes - 16384) * 3 / 4 - 16]) {
        // Keep quota probe ranges separate: this check measures native heap limits, not random allocation collisions.
        const operation = `${((sequence + 1) * 64).toString(16).padStart(8, '0')}-0000-4000-8000-000000000001`;
        const envelope = await sealCheckpoint({ v: 1, collection: grant.collection, document, member: grant.member,
            stream: 'body', sequence: ++sequence, operation, epoch: 1 }, new Uint8Array(bytes), keyBytes(), who);
        const body = JSON.stringify({ collection: grant.collection, member: grant.member, envelope });
        const appended = run('append', body);
        assert.equal(appended.result.statusCode, 200, appended.result.body);
        assert.equal(JSON.parse(appended.result.body).digest, await checkpointDigest(envelope));
        assert.equal((await handle({ path: '/sync/append', method: 'POST', headers: { authorization: `Bearer ${token}` }, body: new TextEncoder().encode(body) }, runtime))?.statusCode, 200);
        const seed = [...values];
        assert.equal(run('append', body, seed).result.statusCode, 200);
        const key = seed.find(([key]: [string]) => key.includes('/updates/') && key.endsWith(envelope.checkpoint.operation))![0];
        const fetched = run('get', JSON.stringify({ collection: grant.collection, member: grant.member, key }), seed);
        assert.equal(fetched.result.statusCode, 200);
        assert.equal(JSON.parse(fetched.result.body).envelope.checkpoint.ciphertext, envelope.checkpoint.ciphertext);
        assert.equal(decode(envelope.checkpoint.ciphertext).length, bytes + 16);
        const renewed = run('touch-record', JSON.stringify({ collection: grant.collection, member: grant.member, document, key }), seed);
        assert.equal(renewed.result.statusCode, 200);
    }
});
