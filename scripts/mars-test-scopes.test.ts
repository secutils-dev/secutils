import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import { test } from 'node:test';
import { closeTestScope, openTestScope, recordSnapshotCanary, scopeRequest, testScopeManifestPath, validateTestOrigin } from '../e2e/mars-test-scopes.ts';

test('test-scope manifests are private, recorded before creation, and retain exact identity across paged cleanup', async (t) => {
    const run = randomUUID(), id = createHash('sha256').update('unit test').digest('hex');
    let manifestPath = '', closes = 0, snapshots = 0;
    const snapshotId = Buffer.alloc(12, 9).toString('base64url'), snapshotToken = Buffer.alloc(16, 9).toString('base64url');
    t.mock.method(globalThis, 'fetch', async (url: string, options: RequestInit) => {
        if (options.method === 'DELETE') {
            assert.equal(url, `http://localhost:7373/?s=${snapshotId}`);
            assert.equal(new Headers(options.headers).get('x-su-delete-token'), snapshotToken);
            snapshots++; return new Response(null, { status: 204 });
        }
        assert.equal(url, 'http://localhost:7373/sync/testing'); assert.equal(options.redirect, 'error');
        const body = JSON.parse(options.body as string);
        assert.equal(body.run, run); assert.equal(body.test, id);
        if (body.operation === 'open') {
            manifestPath = testScopeManifestPath(run, body.scope);
            const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
            assert.equal(manifest.state, 'opening'); assert.equal(manifest.scope, body.scope);
            assert.equal(createHash('sha256').update(manifest.token).digest('hex'), body.tokenHash);
            assert.equal(statSync(manifestPath).mode & 0o777, 0o600);
            return Response.json({ scope: body.scope });
        }
        closes++; return Response.json({ complete: closes === 2, removed: closes === 2 ? 70 : 64 });
    });
    const opened = await openTestScope('http://localhost:7373', run, id, 'Private test record');
    assert.equal(opened.manifest.state, 'active');
    recordSnapshotCanary(opened.path, snapshotId, snapshotToken);
    await assert.rejects(closeTestScope(opened.path, 'https://mars.secutils.dev'), /Invalid MARS cleanup manifest/);
    assert.equal(closes, 0);
    const closed = await closeTestScope(opened.path, 'http://localhost:7373');
    assert.equal(closed.state, 'closed'); assert.equal(closed.removed, 70); assert.equal(closes, 2);
    assert.equal(snapshots, 1);
    assert.equal(statSync(opened.path).mode & 0o777, 0o600);
    assert.equal(JSON.stringify(closed).includes('local-operator-secret'), false);
});

test('cleanup refuses unconfigured origins, path injection, and redirects without sending credentials', async (t) => {
    let requests = 0;
    t.mock.method(globalThis, 'fetch', async () => { requests++; return Response.json({}, { status: 302 }); });
    for (const origin of ['https://mars.secutils.dev.attacker.test', 'http://mars.secutils.dev', 'https://mars.secutils.dev/path', 'https://user:secret@mars.secutils.dev']) {
        assert.throws(() => validateTestOrigin(origin));
        await assert.rejects(scopeRequest(origin, { operation: 'list' }));
    }
    await assert.rejects(openTestScope('http://localhost:7373', '../outside', 'a'.repeat(64), 'Invalid'), /Invalid MARS test identity/);
    assert.equal(requests, 0);
    await assert.rejects(scopeRequest('http://localhost:7373', { operation: 'list' }), /HTTP 302/);
    assert.equal(requests, 1);
});
