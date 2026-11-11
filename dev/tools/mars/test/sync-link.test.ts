import assert from 'node:assert/strict';
import test from 'node:test';
import { identity, openJson } from '../src/sync/crypto.ts';
import { decode, encode, validId } from '../src/sync/protocol.ts';
import { liveAccess } from '../src/sync/live-link.ts';
import { newSyncSecret, openSyncBootstrap, sealSyncBootstrap, syncAccess } from '../src/sync/sync-link.ts';

test('short Sync links keep setup secrets out of requests and bind their encrypted bootstrap', async () => {
    const secret = newSyncSecret(),
        access = await syncAccess(secret),
        who = await identity();
    assert.equal(secret.length, 22);
    assert.equal(validId(access.id), true);
    assert.notEqual(access.token, encode(access.key));
    assert.notEqual(access.id, (await liveAccess(secret)).id);
    const link = {
        v: 1 as const,
        id: access.id,
        secret: access.token,
        ownerKey: who.publicKey,
        wrappingKey: encode(access.key),
    };
    const bootstrap = await sealSyncBootstrap(link, secret);
    assert.deepEqual(await openSyncBootstrap(bootstrap, secret), link);
    assert.equal(JSON.stringify(bootstrap).includes(secret), false);
    await assert.rejects(
        openJson(bootstrap, decode(access.token), { purpose: 'mars-sync-bootstrap-v1', invitation: access.id }),
    );
    await assert.rejects(openSyncBootstrap(bootstrap, newSyncSecret()));
});
