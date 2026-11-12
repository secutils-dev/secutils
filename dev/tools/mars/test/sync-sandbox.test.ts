import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';
import * as vm from 'node:vm';
import { identity, sign, signGrant, randomSecret, tokenHash } from '../src/sync/crypto.ts';
import { createHash } from 'node:crypto';
import { decode, encode, jsonBytes, type Grant } from '../src/sync/protocol.ts';
import type { Kv } from '../src/sync/responder.ts';
import { inlineMarsSync } from '../sync-assets.ts';

test('the compiled responder verifies signatures without browser globals or native Web Crypto', async () => {
    const compiled = await build({
        entryPoints: ['src/sync/responder.ts'],
        bundle: true,
        minify: true,
        format: 'iife',
        globalName: 'MARS_SYNC',
        target: 'es2023',
        write: false,
    });
    const sandbox = vm.createContext({
        Deno: {
            core: {
                encode: (value: string) => new TextEncoder().encode(value),
                decode: (value: Uint8Array) => new TextDecoder().decode(value),
            },
        },
        Uint8Array,
    });
    const api = vm.runInContext(
        `(() => {
        const TextEncoder = class { encode(value) { return Deno.core.encode(value); } };
        const TextDecoder = class { decode(value) { return Deno.core.decode(value); } };
        ${compiled.outputFiles[0].text}
        return MARS_SYNC;
    })()`,
        sandbox,
    );
    assert.equal(vm.runInContext('typeof crypto', sandbox), 'undefined');
    const who = await identity(),
        token = randomSecret(),
        grant: Grant = {
            v: 1,
            collection: crypto.randomUUID(),
            member: crypto.randomUUID(),
            participant: crypto.randomUUID(),
            permission: 'owner',
            document: null,
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(token),
            expires: 0,
        };
    const signed = await signGrant(grant, who);
    assert.equal(api.responderInternals.signatureValid(grant, signed.signature, who.publicKey), true);
    assert.equal(
        api.responderInternals.signatureValid(
            { ...grant, document: crypto.randomUUID() },
            signed.signature,
            who.publicKey,
        ),
        false,
    );
    // The exact sandbox entry, not an optimistic Node-only implementation, serves the limits.
    const forbidden = async () => {
        throw new Error('limits must not touch storage');
    };
    const runtime = { kv: new Proxy({} as Kv, { get: () => forbidden }), crypto: { sha256: forbidden } };
    const response = await api.handle({ path: '/sync/limits', method: 'GET', body: new Uint8Array() }, runtime);
    assert.equal(response.statusCode, 200);
    assert.equal(JSON.parse(response.body).inactiveSeconds, 2592000);
    assert.equal(decode(signed.signature).length, 64);
    // Exercise signed deletion through the actual compiled entry, without Web Crypto in the sandbox.
    const document = crypto.randomUUID(),
        prefix = `sync/v1/c/${grant.collection}/`;
    const values = new Map<string, string>([
        [`${prefix}owner`, encode(jsonBytes({ v: 1, owner: signed, root: { document, kind: 'doc' } }))],
        [`${prefix}members/${grant.member}`, encode(jsonBytes({ signed, slot: `${prefix}member-slots/0` }))],
        [`${prefix}member-slots/0`, encode(jsonBytes(grant.member))],
        [`${prefix}documents/${document}`, encode(jsonBytes({ id: document, kind: 'doc' }))],
    ]);
    let operations = 0;
    const storage: Kv = {
        get: async (key) => {
            operations++;
            return values.get(key) ?? null;
        },
        getEntry: async (key) => ({ value: values.get(key) ?? null, expiresAt: null }),
        set: async (key, value, options) => {
            operations++;
            if (!options?.ifAbsent || !values.has(key)) values.set(key, value);
        },
        delete: async (key) => values.delete(key),
        list: async ({ prefix, limit, after = '' }) => {
            operations++;
            const entries = [...values]
                .filter(([key]) => key.startsWith(prefix) && key > after)
                .slice(0, limit)
                .map(([key, value]) => ({ key, value }));
            return { entries, cursor: entries.at(-1)?.key ?? null };
        },
    };
    const deletion = {
        v: 1,
        kind: 'mars-sync-delete',
        collection: grant.collection,
        document,
        member: grant.member,
        operation: crypto.randomUUID(),
    };
    const receipt = { deletion, signature: await sign(deletion, who), author: signed };
    const run = (action: string, extras: Record<string, unknown> = {}) =>
        api.handle(
            {
                path: `/sync/${action}`,
                method: 'POST',
                headers: { authorization: `Bearer ${token}` },
                body: new TextEncoder().encode(
                    JSON.stringify({ collection: grant.collection, member: grant.member, ...extras }),
                ),
            },
            {
                kv: storage,
                crypto: {
                    sha256: (bytes: string) =>
                        createHash('sha256').update(Buffer.from(bytes, 'base64url')).digest('hex'),
                },
            },
        );
    assert.equal((await run('delete-document', { document, deletion: receipt })).statusCode, 200);
    const deleted = await run('deletions');
    assert.equal(deleted.statusCode, 200);
    assert.deepEqual(JSON.parse(deleted.body).deletions, [receipt]);
    assert.ok(operations < 200);
});

test('responder assembly preserves literal dollar replacement sequences in compiled code', () => {
    const compiled = 'var MARS_SYNC = { handle: () => "literal $&" };';
    const result = inlineMarsSync('(() => { // @su:mars-sync\n return null; })()', compiled);
    assert.ok(result.includes(compiled));
    assert.doesNotThrow(() => new vm.Script(result));
});
