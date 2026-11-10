import assert from 'node:assert/strict';
import test from 'node:test';
import { identity, keyBytes, openJson } from '../src/sync/crypto.ts';
import { decode, encode, validId } from '../src/sync/protocol.ts';
import { liveAccess, newLiveSecret, openLiveBootstrap, sealLiveBootstrap, linkWrappingKey, validLinkWrappingKey } from '../src/sync/live-link.ts';

test('short Live fragments conceal signing/document keys and separate server access from decryption', async () => {
    const secret = newLiveSecret(), access = await liveAccess(secret), delegate = await identity();
    assert.equal(secret.length, 22); assert.equal(validId(access.id), true);
    assert.equal(access.token.length, 43); assert.notEqual(access.token, encode(access.key));
    assert.deepEqual(await liveAccess(secret), access);
    const link = { v: 1 as const, id: access.id, secret: access.token, ownerKey: delegate.publicKey, signing: delegate.signing,
        key: encode(keyBytes()), epoch: 3, wrappingKey: encode(access.key) };
    const bootstrap = await sealLiveBootstrap(link, secret);
    for (const value of [secret, link.key, link.wrappingKey, delegate.signing.d!]) assert.equal(JSON.stringify(bootstrap).includes(value), false);
    assert.deepEqual(await openLiveBootstrap(bootstrap, secret), link);
    await assert.rejects(openLiveBootstrap(bootstrap, newLiveSecret()));
    await assert.rejects(openJson(bootstrap, decode(access.token), { purpose: 'mars-live-bootstrap-v1', invitation: access.id }));
    await assert.rejects(openLiveBootstrap({ ...bootstrap, nonce: encode(new Uint8Array(12)) }, secret));
    await assert.rejects(sealLiveBootstrap({ ...link, id: crypto.randomUUID() }, secret), /Invalid/);
    for (const invalid of ['', 'x'.repeat(21), '='.repeat(22), 'x'.repeat(23)]) await assert.rejects(liveAccess(invalid));
});

test('links require an independent wrapping key and never fall back to their bearer token', () => {
    const secret = encode(keyBytes()), wrappingKey = encode(keyBytes());
    assert.deepEqual(linkWrappingKey({ secret, wrappingKey }), decode(wrappingKey));
    for (const value of [null, {}, { secret }, { secret, wrappingKey: secret }, { secret, wrappingKey: '' },
        { secret, wrappingKey: encode(new Uint8Array(16)) }, { secret, wrappingKey: '*'.repeat(43) }]) {
        assert.equal(validLinkWrappingKey(value), false);
        assert.throws(() => linkWrappingKey(value), /unsupported-link-key-format/);
    }
});
