import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';
import * as vm from 'node:vm';
import { identity, signGrant, randomSecret, tokenHash } from '../src/sync/crypto.ts';
import { decode, type Grant } from '../src/sync/protocol.ts';
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
        grant: Grant = {
            v: 1,
            collection: crypto.randomUUID(),
            member: crypto.randomUUID(),
            participant: crypto.randomUUID(),
            permission: 'owner',
            document: null,
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(randomSecret()),
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
});

test('responder assembly preserves literal dollar replacement sequences in compiled code', () => {
    const compiled = 'var MARS_SYNC = { handle: () => "literal $&" };';
    const result = inlineMarsSync('(() => { // @su:mars-sync\n return null; })()', compiled);
    assert.ok(result.includes(compiled));
    assert.doesNotThrow(() => new vm.Script(result));
});
