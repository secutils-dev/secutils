import assert from 'node:assert/strict';
import test from 'node:test';
import { isConnectionLocation, receiveLocation } from '../src/sync/index.ts';

test('Sync routes recognize new links and preserve legacy pairing and live links', async () => {
    for (const fragment of ['sync=value', 'invite=value', 'live=value', 'sync=']) assert.equal(isConnectionLocation(`https://mars.example/#${fragment}`), true);
    for (const fragment of ['', 'doc=local-id', 'name=Report&md=value', 's=secret', 'b=bundle']) {
        const url = `https://mars.example/#${fragment}`;
        assert.equal(isConnectionLocation(url), false);
        assert.equal(await receiveLocation(url), false);
    }
});
