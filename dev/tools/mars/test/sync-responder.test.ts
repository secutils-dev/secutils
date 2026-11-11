import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { handle, responderInternals, type Kv } from '../src/sync/responder.ts';
import { canonical, decode, encode, LIMITS, type Grant, type SignedCheckpoint } from '../src/sync/protocol.ts';
import {
    identity,
    keyBytes,
    randomSecret,
    sealCheckpoint,
    signGrant,
    tokenHash,
    sign,
    sealJson,
} from '../src/sync/crypto.ts';
import { liveAccess, newLiveSecret, openLiveBootstrap, sealLiveBootstrap } from '../src/sync/live-link.ts';
import { newSyncSecret, syncAccess, sealSyncBootstrap, openSyncBootstrap } from '../src/sync/sync-link.ts';

class TestKv implements Kv {
    values = new Map<string, { value: string; expiresAt: number | null }>();
    time = Math.floor(Date.now() / 1000);
    hook: ((method: string, key: string) => Promise<void>) | null = null;
    async getEntry(key: string) {
        await this.hook?.('getEntry', key);
        const entry = this.values.get(key);
        return entry && (entry.expiresAt === null || entry.expiresAt > this.time)
            ? { ...entry }
            : { value: null, expiresAt: null };
    }
    async get(key: string) {
        return (await this.getEntry(key)).value;
    }
    async set(key: string, value: string, options: { ifAbsent?: boolean; ttlSec?: number } = {}) {
        await this.hook?.('set', key);
        if (decode(value).length > 1024 * 1024) throw new Error('value cap');
        const entry = this.values.get(key),
            live = entry && (entry.expiresAt === null || entry.expiresAt > this.time);
        if (!options.ifAbsent || !live)
            this.values.set(key, { value, expiresAt: options.ttlSec ? this.time + options.ttlSec : null });
    }
    async delete(key: string) {
        await this.hook?.('delete', key);
        return this.values.delete(key);
    }
    async list({ prefix, after = '', limit, valuesIncluded = true }: Parameters<Kv['list']>[0]) {
        await this.hook?.('list', prefix);
        const entries = [...this.values.entries()]
            .filter(
                ([key, entry]) =>
                    key.startsWith(prefix) && key > after && (entry.expiresAt === null || entry.expiresAt > this.time),
            )
            .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
            .slice(0, Math.min(limit, 1000))
            .map(([key, entry]) => ({ key, ...(valuesIncluded ? { value: entry.value } : {}) }));
        return { entries, cursor: entries.at(-1)?.key ?? null };
    }
}
const id = () => crypto.randomUUID();
async function fixture(root: boolean | 'folder' = false, kv = new TestKv()) {
    const who = await identity(),
        token = randomSecret();
    const owner: Grant = {
        v: 1,
        collection: id(),
        member: id(),
        participant: id(),
        permission: 'owner',
        document: null,
        publicKey: who.publicKey,
        boxKey: who.boxKey,
        tokenHash: await tokenHash(token),
        expires: 0,
    };
    const signed = await signGrant(owner, who),
        document = id(),
        dataKey = keyBytes();
    const request = async (
        action: string,
        body: Record<string, unknown> = {},
        bearer = token,
        clientAddress = '127.0.0.1:0',
    ) => {
        let operations = 0;
        const bounded = new Proxy(kv, {
            get(target, property) {
                const value = Reflect.get(target, property);
                return typeof value !== 'function'
                    ? value
                    : (...args: unknown[]) => {
                          if (++operations > 200) throw new Error('operation budget');
                          return value.apply(target, args);
                      };
            },
        });
        const response = await handle(
            {
                path: `/sync/${action}`,
                method: 'POST',
                headers: { authorization: `Bearer ${bearer}` },
                body: new TextEncoder().encode(
                    JSON.stringify({ collection: owner.collection, member: owner.member, ...body }),
                ),
                clientAddress,
                params: { secrets: { MARS_STATS_API_KEY: 'operator-secret' } },
            },
            {
                kv: bounded,
                crypto: {
                    sha256: (value) => createHash('sha256').update(Buffer.from(value, 'base64url')).digest('hex'),
                },
            },
        );
        assert.equal(response?.skipRequest, true);
        return { status: response!.statusCode, body: response!.body ? JSON.parse(response!.body) : null, operations };
    };
    const kind = root === 'folder' ? 'folder' : 'doc';
    assert.equal(
        (await request('create', { owner: signed, ...(root ? { root: { document, kind } } : {}) })).status,
        201,
    );
    assert.equal((await request('document', { document, kind })).status, 201);
    const checkpoint = (
        sequence: number,
        operation = id(),
        stream: 'body' | 'review' | 'tree' = 'body',
        bytes = new TextEncoder().encode(`state ${sequence}`),
    ) =>
        sealCheckpoint(
            {
                v: 1,
                collection: owner.collection,
                document,
                member: owner.member,
                stream,
                sequence,
                operation,
                epoch: 1,
            },
            bytes,
            dataKey,
            who,
        );
    return { kv, who, token, owner, signed, document, dataKey, request, checkpoint };
}

test('collection creation permits 50 per IP per hour, reports a distinct error, and does not block existing sync', async (t) => {
    let now = Math.floor(Date.now() / 3600000) * 3600 + 1;
    t.mock.method(Date, 'now', () => now * 1000);
    const f = await fixture();
    const owner = () => signGrant({ ...f.owner, collection: id(), member: id() }, f.who);
    for (let i = 1; i < LIMITS.collections; i++)
        assert.equal((await f.request('create', { owner: await owner() })).status, 201);
    assert.equal((await f.request('create', { owner: f.signed })).status, 201);
    const rejected = await f.request('create', { owner: await owner() }, f.token, '127.0.0.1:1234');
    assert.equal(rejected.status, 429);
    assert.equal(rejected.body.error, 'collection-creation-limit');
    assert.equal((await f.request('create', { owner: await owner() }, f.token, '127.0.0.2:1234')).status, 201);
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    const limits = await handle(
        { path: '/sync/limits', method: 'GET', body: new Uint8Array() },
        {
            kv: f.kv,
            crypto: { sha256: (value) => createHash('sha256').update(Buffer.from(value, 'base64url')).digest('hex') },
        },
    );
    assert.equal(JSON.parse(limits!.body).collections, 50);
    now += 3600;
    f.kv.time = now;
    assert.equal((await f.request('create', { owner: await owner() })).status, 201);
});

test('the folder-item limit stays at 100 and is distinct from the hourly collection limit', async () => {
    const f = await fixture();
    for (let i = 1; i < LIMITS.documents; i++)
        assert.equal((await f.request('document', { document: id(), kind: i % 2 ? 'doc' : 'folder' })).status, 201);
    const rejected = await f.request('document', { document: id(), kind: 'doc' });
    assert.equal(rejected.status, 507);
    assert.equal(rejected.body.error, 'membership-or-document-limit');
});

test('an index page can include the complete directory within the KV budget without exposing it to document-only grants', async () => {
    const f = await fixture('folder');
    for (let i = 1; i < LIMITS.documents; i++)
        assert.equal((await f.request('document', { document: id(), kind: i % 2 ? 'doc' : 'folder' })).status, 201);
    const page = await f.request('index', { directory: true });
    assert.equal(page.status, 200);
    assert.equal(page.body.directory.documents.length, LIMITS.documents);
    assert.ok(page.operations <= 200);
    assert.equal((await f.request('index')).body.directory, undefined);
    const who = await identity(),
        token = randomSecret(),
        grant = await signGrant(
            {
                ...f.owner,
                member: id(),
                participant: id(),
                permission: 'view',
                document: f.document,
                expires: Math.floor(Date.now() / 1000) + 86400,
                publicKey: who.publicKey,
                boxKey: who.boxKey,
                tokenHash: await tokenHash(token),
            },
            f.who,
        );
    assert.equal((await f.request('grant', { signed: grant })).status, 201);
    assert.equal((await f.request('index', { member: grant.grant.member, directory: true }, token)).status, 403);
});

test('opening a folder can renew nested folder metadata along with unopened document replicas', async (t) => {
    let now = Math.floor(Date.now() / 1000);
    t.mock.method(Date, 'now', () => now * 1000);
    const f = await fixture('folder'),
        folder = id(),
        document = id();
    await f.request('document', { document: folder, kind: 'folder' });
    await f.request('document', { document, kind: 'doc' });
    await f.request('append', { envelope: await f.checkpoint(1, id(), 'tree') });
    const leaf = await sealCheckpoint(
        {
            v: 1,
            collection: f.owner.collection,
            document,
            member: f.owner.member,
            stream: 'body',
            sequence: 1,
            operation: id(),
            epoch: 1,
        },
        new TextEncoder().encode('leaf state'),
        f.dataKey,
        f.who,
    );
    await f.request('append', { envelope: leaf });
    now += 29 * 86400;
    f.kv.time = now;
    assert.equal((await f.request('touch-folder', { document: folder })).status, 409);
    const renew = async (document: string) => {
        for (const entry of (await f.request('index', { document })).body.entries)
            assert.equal((await f.request('touch-record', { document, key: entry.key })).status, 200);
        assert.equal((await f.request('touch', { document })).status, 200);
    };
    await renew(f.document);
    assert.equal((await f.request('touch-folder', { document: folder })).status, 200);
    await renew(document);
    now += 2 * 86400;
    f.kv.time = now;
    const directory = await f.request('directory');
    assert.equal(directory.status, 200);
    assert.equal(directory.body.documents.length, 3);
    assert.equal((await f.request('index', { document })).body.entries.length, 1);
});

test('a short Sync bootstrap authenticates separately and expires after ten minutes', async (t) => {
    let now = Math.floor(Date.now() / 1000);
    t.mock.method(Date, 'now', () => now * 1000);
    const f = await fixture(true),
        fragment = newSyncSecret(),
        access = await syncAccess(fragment);
    const invitation = {
        v: 1 as const,
        id: access.id,
        collection: f.owner.collection,
        document: null,
        mode: 'pair' as const,
        permission: 'edit' as const,
        publicKey: f.who.publicKey,
        participant: f.owner.participant,
        expires: now + LIMITS.invitationSeconds,
    };
    const link = {
        v: 1 as const,
        id: access.id,
        secret: access.token,
        ownerKey: f.who.publicKey,
        wrappingKey: encode(access.key),
    };
    const bootstrap = await sealSyncBootstrap(link, fragment);
    assert.equal(
        (
            await f.request('invite', {
                policy: { invitation, signature: await sign(invitation, f.who) },
                tokenHash: await tokenHash(access.token),
                bootstrap,
            })
        ).status,
        201,
    );
    assert.equal((await f.request('invite-read', { invitation: access.id }, randomSecret())).status, 403);
    const response = await f.request('invite-read', { invitation: access.id }, access.token);
    assert.deepEqual(await openSyncBootstrap(response.body.bootstrap, fragment), link);
    now += 601;
    f.kv.time = now;
    assert.equal((await f.request('invite-read', { invitation: access.id }, access.token)).status, 410);
});

test('Live-only cleanup removes encrypted storage and reclaims quotas in bounded, repeatable requests', async () => {
    const f = await fixture(true),
        secret = randomSecret(),
        invitation = {
            v: 1 as const,
            id: id(),
            collection: f.owner.collection,
            document: f.document,
            mode: 'live' as const,
            permission: 'edit' as const,
            publicKey: f.who.publicKey,
            participant: null,
            expires: Math.floor(Date.now() / 1000) + 604800,
        };
    await f.request('invite', {
        policy: { invitation, signature: await sign(invitation, f.who) },
        tokenHash: await tokenHash(secret),
        bootstrap: { nonce: encode(new Uint8Array(12)), ciphertext: encode(new Uint8Array(32)) },
    });
    for (const stream of ['body', 'review'] as const)
        assert.equal(
            (await f.request('append', { envelope: await f.checkpoint(1, id(), stream, new Uint8Array(200 * 1024)) }))
                .status,
            200,
        );
    assert.ok((await f.request('telemetry', {}, 'operator-secret')).body.reservedBytes > 0);
    let after: string | undefined;
    for (let pass = 0; ; pass++) {
        const result = await f.request('purge-live-document', { document: f.document, after });
        assert.equal(result.status, 200, canonical(result));
        assert.ok(result.operations <= 200);
        if (result.body.complete) break;
        after = result.body.after;
        assert.ok(pass < 100);
    }
    assert.equal((await f.request('telemetry', {}, 'operator-secret')).body.reservedBytes, 0);
    assert.equal((await f.request('invite-read', { invitation: invitation.id }, secret)).status, 410);
    assert.equal((await f.request('purge-live-document', { document: f.document })).status, 410);
    assert.equal(
        [...f.kv.values.keys()].some(
            (key) =>
                key.includes(`/payloads/${f.document}/`) ||
                key.includes(`/updates/${f.document}/`) ||
                key.includes(`/invitations/${invitation.id}`),
        ),
        false,
    );
});

test('Live cleanup refuses to delete a document still used by a paired device', async () => {
    const f = await fixture(true);
    const member = await signGrant({ ...f.owner, member: id(), permission: 'edit' }, f.who);
    assert.equal((await f.request('grant', { signed: member })).status, 201);
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    const result = await f.request('purge-live-document', { document: f.document });
    assert.equal(result.status, 409);
    assert.equal(result.body.error, 'personal-sync-active');
    assert.equal((await f.request('index')).body.entries.length, 1);
});

test('a key write racing with document deletion cannot leave new encrypted key material behind', async () => {
    const f = await fixture(true),
        wrapped = {
            v: 1 as const,
            collection: f.owner.collection,
            document: f.document,
            epoch: 2,
            member: f.owner.member,
            sender: f.who.boxKey,
            nonce: encode(new Uint8Array(12)),
            ciphertext: encode(new Uint8Array(48)),
        };
    let deleted = false;
    f.kv.hook = async (method, key) => {
        if (!deleted && method === 'set' && key.startsWith(`sync/v1/c/${f.owner.collection}/keys/`)) {
            deleted = true;
            assert.equal((await f.request('delete-document', { document: f.document })).status, 200);
        }
    };
    const result = await f.request('key-write', { values: [{ wrapped, signature: await sign(wrapped, f.who) }] });
    assert.equal(result.status, 410);
    assert.equal(
        [...f.kv.values.keys()].some((key) => key.startsWith(`sync/v1/c/${f.owner.collection}/keys/`)),
        false,
    );
});

test('encrypted Live bootstrap supports all lifetimes, exact retries, authentication, revocation, and expiry', async (t) => {
    const now = Math.floor(Date.now() / 1000);
    t.mock.method(Date, 'now', () => now * 1000);
    const f = await fixture();
    for (const days of [1, 7, 14, 30]) {
        const secret = newLiveSecret(),
            access = await liveAccess(secret),
            delegate = await identity();
        const invitation = {
            v: 1 as const,
            id: access.id,
            collection: f.owner.collection,
            document: f.document,
            mode: 'live' as const,
            permission: 'edit' as const,
            publicKey: delegate.publicKey,
            participant: null,
            expires: now + days * 86400,
        };
        const policy = { invitation, signature: await sign(invitation, f.who) };
        const link = {
            v: 1 as const,
            id: access.id,
            secret: access.token,
            ownerKey: f.who.publicKey,
            signing: delegate.signing,
            key: encode(f.dataKey),
            epoch: 1,
            wrappingKey: encode(access.key),
        };
        const bootstrap = await sealLiveBootstrap(link, secret),
            request = { policy, tokenHash: await tokenHash(access.token), bootstrap };
        assert.equal((await f.request('invite', request)).status, 201);
        assert.equal((await f.request('invite', request)).status, 201);
        assert.equal((await f.request('invite-read', { invitation: access.id }, randomSecret())).status, 403);
        const read = await f.request('invite-read', { invitation: access.id }, access.token);
        assert.equal(read.status, 200);
        assert.deepEqual(await openLiveBootstrap(read.body.bootstrap, secret), link);
        const stored = await f.kv.get(`sync/v1/invitations/${access.id}`);
        const plain = new TextDecoder().decode(decode(stored!));
        for (const privateValue of [secret, link.key, link.wrappingKey, delegate.signing.d!])
            assert.equal(plain.includes(privateValue), false);
        assert.equal(
            (await f.request('invite', { ...request, bootstrap: { ...bootstrap, ciphertext: 'x'.repeat(16385) } }))
                .status,
            400,
        );
        f.kv.time = invitation.expires;
        assert.equal((await f.request('invite-read', { invitation: access.id }, access.token)).status, 410);
        f.kv.time = now;
        assert.equal((await f.request('invite-revoke', { invitation: access.id })).status, 200);
        assert.equal((await f.request('invite-read', { invitation: access.id }, access.token)).status, 410);
        assert.equal((await f.request('invite', request)).status, 403);
    }
});

test('immutable checkpoint retry acknowledges identical bytes and rejects operation reuse', async () => {
    const f = await fixture(),
        envelope = await f.checkpoint(1);
    const first = await f.request('append', { envelope }),
        again = await f.request('append', { envelope });
    assert.equal(first.status, 200);
    assert.deepEqual(first.body, again.body);
    assert.equal(
        (await f.request('append', { envelope: await f.checkpoint(2, envelope.checkpoint.operation) })).status,
        409,
    );
    const index = await f.request('index');
    assert.equal(index.body.entries.length, 1);
    const stored = await f.request('get', { key: index.body.entries[0].key });
    assert.deepEqual(stored.body.envelope, envelope);
});

test('rotation activation and invitation revocation survive duplicate requests without exposing unactivated keys', async () => {
    const f = await fixture(),
        delegate = await identity(),
        secret = randomSecret();
    const invitation = {
        v: 1 as const,
        id: id(),
        collection: f.owner.collection,
        document: f.document,
        mode: 'live' as const,
        permission: 'view' as const,
        publicKey: delegate.publicKey,
        participant: null,
        expires: Math.floor(Date.now() / 1000) + 600,
    };
    const policy = { invitation, signature: await sign(invitation, f.who) };
    assert.equal((await f.request('invite', { policy, tokenHash: await tokenHash(secret) })).status, 201);
    for (const epoch of [1, 2]) {
        const value = {
            document: f.document,
            epoch,
            ...(await sealJson({ epoch, key: randomSecret() }, decode(secret), { invitation: invitation.id })),
        };
        assert.equal(
            (
                await f.request('live-key-write', {
                    invitation: invitation.id,
                    value,
                    signature: await sign(value, f.who),
                })
            ).status,
            200,
        );
    }
    assert.equal((await f.request('invite-read', { invitation: invitation.id }, secret)).body.keyUpdate.value.epoch, 1);
    for (let retry = 0; retry < 2; retry++)
        assert.equal((await f.request('rotate-document', { document: f.document, epoch: 2 })).status, 200);
    assert.equal((await f.request('invite-read', { invitation: invitation.id }, secret)).body.keyUpdate.value.epoch, 2);
    assert.equal((await f.request('rotate-document', { document: f.document, epoch: 1 })).status, 409);
    for (let retry = 0; retry < 2; retry++)
        assert.equal((await f.request('invite-revoke', { invitation: invitation.id })).status, 200);
    assert.equal((await f.request('invite', { policy, tokenHash: await tokenHash(secret) })).status, 403);
});

test('lost acknowledgements and pruning never overwrite the newest full checkpoint', async () => {
    const f = await fixture(),
        first = await f.checkpoint(1);
    for (let sequence = 1; sequence <= 5; sequence++)
        assert.equal(
            (await f.request('append', { envelope: sequence === 1 ? first : await f.checkpoint(sequence) })).status,
            200,
        );
    const index = await f.request('index');
    assert.equal(index.body.entries.length, 2);
    assert.equal((await f.request('append', { envelope: first })).status, 200);
    assert.equal((await f.request('append', { envelope: await f.checkpoint(2) })).status, 409);
    assert.equal((await f.request('get', { key: index.body.entries[0].key })).body.envelope.checkpoint.sequence, 5);
});

test('body edits, identity forgery, sibling reads, and scope escape fail at the responder', async () => {
    const f = await fixture(),
        who = await identity(),
        token = randomSecret(),
        grant: Grant = {
            ...f.owner,
            member: id(),
            participant: id(),
            permission: 'comment',
            document: f.document,
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(token),
            expires: Math.floor(Date.now() / 1000) + 600,
        };
    assert.equal((await f.request('grant', { signed: await signGrant(grant, f.who) })).status, 201);
    const envelope = await sealCheckpoint(
        {
            v: 1,
            collection: grant.collection,
            document: f.document,
            member: grant.member,
            stream: 'body',
            sequence: 1,
            operation: id(),
            epoch: 1,
        },
        new Uint8Array([1, 2, 3]),
        f.dataKey,
        who,
    );
    const asGuest = { member: grant.member };
    assert.equal((await f.request('append', { ...asGuest, envelope }, token)).status, 403);
    assert.equal((await f.request('append', { ...asGuest, envelope: await f.checkpoint(1) }, token)).status, 403);
    assert.equal(
        (await f.request('get', { ...asGuest, key: `sync/v1/c/${grant.collection}/updates/${id()}/other` }, token))
            .status,
        403,
    );
    assert.equal(
        (await f.request('index', { ...asGuest, after: `sync/v1/c/${grant.collection}/updates/${id()}/other` }, token))
            .status,
        400,
    );
    assert.equal(
        (
            await f.request(
                'grant',
                { ...asGuest, signed: await signGrant({ ...grant, permission: 'edit' }, who) },
                token,
            )
        ).status,
        403,
    );
});

test('revoked credentials cannot append, bootstrap, or restore their grant', async () => {
    const f = await fixture(),
        who = await identity(),
        token = randomSecret(),
        grant: Grant = {
            ...f.owner,
            member: id(),
            permission: 'edit',
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(token),
        };
    const signed = await signGrant(grant, f.who);
    assert.equal((await f.request('grant', { signed })).status, 201);
    assert.equal((await f.request('revoke', { target: grant.member })).status, 200);
    assert.equal((await f.request('index', { member: grant.member }, token)).status, 403);
    assert.equal((await f.request('grant', { signed })).status, 403);
});

test('index pagination cannot lose a delayed, lower-sequence checkpoint behind a time cursor', async () => {
    const f = await fixture();
    const higher = await f.checkpoint(2);
    assert.equal((await f.request('append', { envelope: higher })).status, 200);
    // An older sequence from this same writer cannot replace its current checkpoint.
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 409);
    // Polls rescan immutable key indexes, rather than asking for updates after Date.now().
    const first = await f.request('index'),
        second = await f.request('index');
    assert.deepEqual(first.body.entries, second.body.entries);
});

test('large envelopes obey both the 1 MiB KV cap and the 200-operation invocation budget', async () => {
    const f = await fixture(true);
    const envelope = await f.checkpoint(1, id(), 'body', new Uint8Array(512 * 1024));
    const result = await f.request('append', { envelope });
    assert.equal(result.status, 200, canonical(result));
    assert.ok(result.operations <= 200);
    assert.equal(
        (await f.request('append', { envelope: await f.checkpoint(2, id(), 'body', new Uint8Array(1024 * 1024)) }))
            .status,
        413,
    );
});

test('payload-first storage cannot publish an incomplete checkpoint or acknowledge different ciphertext', async () => {
    const f = await fixture(),
        envelope = await f.checkpoint(1);
    let rejectHeader = true;
    f.kv.hook = async (method, key) => {
        if (rejectHeader && method === 'set' && key.includes('/updates/')) throw new Error('Test header failure');
    };
    assert.notEqual((await f.request('append', { envelope })).status, 200);
    assert.equal((await f.request('index')).body.entries.length, 0);
    assert.equal([...f.kv.values.keys()].filter((key) => key.includes('/payloads/')).length, 1);
    rejectHeader = false;
    assert.equal((await f.request('append', { envelope })).status, 200);
    const key = (await f.request('index')).body.entries[0].key;
    const header = JSON.parse(new TextDecoder().decode(decode(f.kv.values.get(key)!.value)));
    assert.equal(header.envelope.checkpoint.ciphertext, '');
    assert.deepEqual((await f.request('get', { key })).body.envelope, envelope);
    f.kv.values.delete(header.payload);
    assert.equal((await f.request('get', { key })).status, 404);
    assert.equal((await f.request('touch-record', { document: f.document, key })).status, 410);
});

test('renewal extends a lease only after the required checkpoints survive to its new expiry', async () => {
    const f = await fixture();
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    const before = await f.request('index'),
        key = before.body.entries[0].key;
    // Native time is authoritative, including a deliberate near-expiry checkpoint.
    f.kv.values.get(key)!.expiresAt = Math.floor(Date.now() / 1000) + 100;
    assert.equal((await f.request('touch', { document: f.document })).status, 409);
    assert.equal((await f.request('touch-record', { document: f.document, key })).status, 200);
    const renewed = await f.request('touch', { document: f.document });
    assert.equal(renewed.status, 200);
    assert.ok(renewed.body.expires <= f.kv.values.get(key)!.expiresAt!);
    assert.equal((await f.request('index')).body.entries.length, 1);
});

test('expiry is a disconnection, not bootstrap of an empty document or implicit recreation', async () => {
    const f = await fixture();
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    f.kv.time += LIMITS.inactiveSeconds + 1;
    assert.equal((await f.request('index')).status, 410);
    assert.equal((await f.request('append', { envelope: await f.checkpoint(2) })).status, 410);
});

test('a delayed duplicate quota release cannot release a later allocation of the same slot', async () => {
    const kv = new TestKv(),
        collection = id(),
        one = '00000000-0000-4000-8000-000000000001',
        two = '00000000-0000-4000-8000-000000000002';
    const first = await responderInternals.reserve(kv, collection, one, 1024);
    let entered!: () => void, resume!: () => void;
    const waiting = new Promise<void>((resolve) => {
        entered = resolve;
    });
    const gate = new Promise<void>((resolve) => {
        resume = resolve;
    });
    let paused = false;
    kv.hook = async (method, key) => {
        if (method !== 'set' || !key.startsWith('sync/v1/budget/all/') || paused) return;
        paused = true;
        entered();
        await gate;
    };
    const delayed = responderInternals.release(kv, first, one);
    await waiting;
    kv.hook = null;
    await responderInternals.release(kv, first, one);
    const second = await responderInternals.reserve(kv, collection, two, 1024);
    assert.equal(second[0].slice(0, second[0].lastIndexOf('/')), first[0].slice(0, first[0].lastIndexOf('/')));
    resume();
    await delayed;
    for (const key of second) {
        const prefix = key.slice(0, key.lastIndexOf('/') + 1),
            head = (await kv.list({ prefix, limit: 1 })).entries[0];
        assert.equal(head.key, key);
        assert.equal(JSON.parse(new TextDecoder().decode(decode(head.value!))).operation, two);
        assert.equal(JSON.parse(new TextDecoder().decode(decode(head.value!))).busy, true);
    }
});

test('telemetry reports reserved capacity, not estimated stored bytes, without private identifiers', async () => {
    const f = await fixture();
    assert.equal((await f.request('telemetry')).status, 403);
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    const telemetry = await f.request('telemetry', {}, 'operator-secret');
    assert.equal(telemetry.status, 200);
    assert.equal(telemetry.body.reservedBytes, 65536);
    assert.equal(telemetry.body.collectionsWithReservations, 1);
    const output = JSON.stringify(telemetry.body);
    for (const secret of [f.owner.collection, f.owner.member, f.document, f.token, f.who.publicKey, 'state 1'])
        assert.equal(output.includes(secret), false);
});

test('cleanup removes stale reservation references after a lost release without freeing a later claimant', async () => {
    const kv = new TestKv(),
        collection = id(),
        otherCollection = id(),
        one = '00000000-0000-4000-8000-000000000011',
        two = '00000000-0000-4000-8000-000000000012';
    const first = await responderInternals.reserve(kv, collection, one, 1024);
    kv.hook = async (method, key) => {
        if (method === 'delete' && key.startsWith(`sync/v1/reservations/${collection}/${one}/`))
            throw new Error('lost release response');
    };
    await assert.rejects(responderInternals.release(kv, first, one), /lost release/);
    kv.hook = null;
    const second = await responderInternals.reserve(kv, otherCollection, two, 1024);
    await responderInternals.release(kv, first, one, 8, collection);
    assert.equal(
        (await kv.list({ prefix: `sync/v1/reservations/${collection}/${one}/`, limit: 10 })).entries.length,
        0,
    );
    for (const key of second) {
        const head = (await kv.list({ prefix: key.slice(0, key.lastIndexOf('/') + 1), limit: 1 })).entries[0];
        assert.equal(head.key, key);
        assert.equal(JSON.parse(new TextDecoder().decode(decode(head.value!))).busy, true);
    }
});

test('maintenance reclaims expired checkpoint permits in bounded requests', async () => {
    const f = await fixture();
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    f.kv.time += LIMITS.inactiveSeconds + 1;
    // Mirror server time on reservation metadata, without changing the test process clock.
    for (const [key, entry] of f.kv.values) {
        if (!key.startsWith('sync/v1/budget/')) continue;
        const value = JSON.parse(new TextDecoder().decode(decode(entry.value)));
        value.reservedAt -= LIMITS.inactiveSeconds + 1;
        entry.value = encode(new TextEncoder().encode(JSON.stringify(value)));
    }
    const result = await f.request('maintenance', {}, 'operator-secret');
    assert.equal(result.status, 200);
    assert.equal(result.body.reclaimedOperations, 1);
    assert.ok(result.operations <= 200);
    assert.equal((await f.request('telemetry', {}, 'operator-secret')).body.reservedBytes, 0);
});

test('simultaneous reuse of an operation ID cannot allocate one permit for two different streams', async () => {
    const f = await fixture(),
        operation = id();
    const [body, review] = await Promise.all([
        f.checkpoint(1, operation, 'body'),
        f.checkpoint(1, operation, 'review'),
    ]);
    const results = await Promise.all([
        f.request('append', { envelope: body }),
        f.request('append', { envelope: review }),
    ]);
    assert.deepEqual(results.map((result) => result.status).sort(), [200, 409]);
    assert.equal((await f.request('index')).body.entries.length, 1);
    assert.equal((await f.request('telemetry', {}, 'operator-secret')).body.reservedBytes, 65536);
});

test('an operator lifespan cap blocks the protocol instead of silently expiring reusable quota ownership', async () => {
    const f = await fixture(),
        original = f.kv.set.bind(f.kv);
    f.kv.set = async (key, value, options = {}) =>
        original(key, value, { ...options, ttlSec: options.ttlSec ?? LIMITS.inactiveSeconds });
    const result = await f.request('append', { envelope: await f.checkpoint(1) });
    assert.equal(result.status, 503);
    assert.equal(result.body.error, 'persistent-permits-unavailable');
    assert.equal((await f.request('index')).body.entries.length, 0);
});

test('large checkpoints defer compaction to bounded maintenance without losing the current state', async () => {
    const f = await fixture(true),
        bytes = new Uint8Array(740 * 1024);
    for (let sequence = 1; sequence <= 3; sequence++) {
        const envelope = await f.checkpoint(sequence, id(), 'body', bytes);
        let result = await f.request('append', { envelope });
        // Random operation IDs can collide with occupied quota slots. Bounded
        // allocation retains partial ownership and advances on an exact retry.
        for (
            let attempt = 0;
            result.status === 507 && result.body.error === 'sync-storage-full' && attempt < 5;
            attempt++
        ) {
            assert.ok(result.operations <= 200);
            result = await f.request('append', { envelope });
        }
        assert.equal(result.status, 200, canonical(result));
        assert.ok(result.operations <= 200);
    }
    let after: string | null = null;
    for (let pass = 0; pass < 100; pass++) {
        const result = await f.request('maintenance', { after }, 'operator-secret');
        assert.equal(result.status, 200, canonical(result));
        assert.ok(result.operations <= 200);
        if (result.body.complete) break;
        after = result.body.cursor;
        assert.ok(pass < 99, 'maintenance must finish its bounded pass');
    }
    const index = await f.request('index');
    assert.equal(index.body.entries.length, 2);
    assert.equal((await f.request('get', { key: index.body.entries[0].key })).body.envelope.checkpoint.sequence, 3);
});

// ---- Hardening regressions ----
// Each case below is an attack or a failure that the responder accepted before its fix.
type Party = Awaited<ReturnType<typeof fixture>>;
async function liveInvite(
    p: Party,
    invitationId = id(),
    secret = randomSecret(),
    permission: 'view' | 'comment' | 'edit' = 'view',
) {
    const delegate = await identity();
    const invitation = {
        v: 1 as const,
        id: invitationId,
        collection: p.owner.collection,
        document: p.document,
        mode: 'live' as const,
        permission,
        publicKey: delegate.publicKey,
        participant: null,
        expires: p.kv.time + 7 * 86400,
    };
    const policy = { invitation, signature: await sign(invitation, p.who) };
    const result = await p.request('invite', {
        policy,
        tokenHash: await tokenHash(secret),
        bootstrap: { nonce: encode(new Uint8Array(12)), ciphertext: encode(new Uint8Array(32)) },
    });
    return { result, policy, secret, delegate, invitation };
}
async function liveJoin(p: Party, link: Awaited<ReturnType<typeof liveInvite>>) {
    const who = await identity(),
        token = randomSecret(),
        member = id();
    const grant: Grant = {
        v: 1,
        collection: p.owner.collection,
        member,
        participant: member,
        permission: link.invitation.permission,
        document: p.document,
        publicKey: who.publicKey,
        boxKey: who.boxKey,
        tokenHash: await tokenHash(token),
        expires: link.invitation.expires,
    };
    const signed = { ...(await signGrant(grant, link.delegate)), issuer: link.policy };
    const result = await p.request('live-join', { invitation: link.invitation.id, signed }, link.secret);
    return { result, token, member };
}

test('a revoked Live link stays revoked when another collection creates an invitation with the same id', async () => {
    const kv = new TestKv(),
        victim = await fixture(true, kv),
        other = await fixture(true, kv);
    const link = await liveInvite(victim),
        guest = await liveJoin(victim, link);
    assert.equal(guest.result.status, 201);
    assert.equal((await victim.request('index', { member: guest.member }, guest.token)).status, 200);
    assert.equal((await victim.request('invite-revoke', { invitation: link.invitation.id })).status, 200);
    assert.equal((await victim.request('index', { member: guest.member }, guest.token)).status, 403);
    // The id is free again in the shared namespace, so another collection can take it.
    assert.equal((await liveInvite(other, link.invitation.id)).result.status, 201);
    const again = await victim.request('index', { member: guest.member }, guest.token);
    assert.equal(again.status, 403);
    assert.equal(again.body.error, 'live-link-ended');
});

test('cleanup of one collection never deletes an invitation of another collection', async () => {
    const kv = new TestKv(),
        victim = await fixture(true, kv),
        other = await fixture(true, kv);
    const link = await liveInvite(victim),
        guest = await liveJoin(victim, link);
    // The colliding id is refused, and it leaves no slot that cleanup would follow.
    const collision = await liveInvite(other, link.invitation.id);
    assert.equal(collision.result.status, 409);
    assert.equal(
        [...kv.values.keys()].some((key) => key.includes(`/c/${other.owner.collection}/invitation-slots/`)),
        false,
    );
    // Even with a slot that names the foreign id, cleanup leaves the foreign records alone.
    await kv.set(
        `sync/v1/c/${other.owner.collection}/invitation-slots/000`,
        encode(new TextEncoder().encode(JSON.stringify(link.invitation.id))),
    );
    for (let pass = 0; pass < 20; pass++) {
        const purge = await other.request('purge-live-document', { document: other.document });
        assert.equal(purge.status, 200);
        if (purge.body.complete) break;
    }
    assert.ok(await kv.get(`sync/v1/invitations/${link.invitation.id}`));
    assert.equal((await victim.request('index', { member: guest.member }, guest.token)).status, 200);
});

test('an append that lost its receipt succeeds on retry, also for a checkpoint with several capacity slots', async () => {
    const f = await fixture();
    for (let attempt = 1; attempt <= 12; attempt++) {
        const envelope = await f.checkpoint(attempt, id(), 'body', new Uint8Array(200 * 1024));
        let lose = true;
        f.kv.hook = async (method, key) => {
            if (lose && method === 'set' && key.includes('/receipts/')) throw new Error('lost');
        };
        assert.equal((await f.request('append', { envelope })).status, 503);
        lose = false;
        f.kv.hook = null;
        assert.equal((await f.request('append', { envelope })).status, 200, `retry ${attempt}`);
    }
});

test('a rotated key epoch lives as long as its document, and device keys are renewed with it', async (t) => {
    let now = Math.floor(Date.now() / 1000);
    t.mock.method(Date, 'now', () => now * 1000);
    const f = await fixture();
    f.kv.time = now;
    assert.equal((await f.request('append', { envelope: await f.checkpoint(1) })).status, 200);
    const wrapped = {
        v: 1 as const,
        collection: f.owner.collection,
        document: f.document,
        epoch: 2,
        member: f.owner.member,
        sender: f.who.boxKey,
        nonce: encode(new Uint8Array(12)),
        ciphertext: encode(new Uint8Array(48)),
    };
    assert.equal(
        (await f.request('key-write', { values: [{ wrapped, signature: await sign(wrapped, f.who) }] })).status,
        200,
    );
    assert.equal((await f.request('rotate-document', { document: f.document, epoch: 2 })).status, 200);
    const atEpoch2 = (sequence: number) =>
        sealCheckpoint(
            {
                v: 1,
                collection: f.owner.collection,
                document: f.document,
                member: f.owner.member,
                stream: 'body',
                sequence,
                operation: id(),
                epoch: 2,
            },
            new Uint8Array([1]),
            f.dataKey,
            f.who,
        );
    // The document stays active for 60 days: an append and a renewal every ten days.
    for (let day = 10; day <= 60; day += 10) {
        now += 10 * 86400;
        f.kv.time = now;
        assert.equal(
            (await f.request('append', { envelope: await atEpoch2(day) })).status,
            200,
            `append on day ${day}`,
        );
        for (const entry of (await f.request('index', { document: f.document })).body.entries)
            await f.request('touch-record', { document: f.document, key: entry.key });
        assert.equal((await f.request('touch', { document: f.document })).status, 200);
        assert.equal((await f.request('epoch', { document: f.document })).body.epoch, 2);
        assert.equal(
            (await f.request('key-read', { document: f.document })).body.values.length,
            1,
            `device key on day ${day}`,
        );
    }
});

test('owner writes without a quota reservation are bounded in size, shape, and target', async () => {
    const f = await fixture(true),
        before = f.kv.values.size;
    const wrapped = {
        v: 1 as const,
        collection: f.owner.collection,
        document: f.document,
        epoch: 2,
        member: f.owner.member,
        sender: f.who.boxKey,
        nonce: encode(new Uint8Array(12)),
        ciphertext: encode(new Uint8Array(48)),
    };
    // An extra field would ride along into storage.
    assert.equal(
        (
            await f.request('key-write', {
                values: [{ wrapped, signature: await sign(wrapped, f.who), junk: 'A'.repeat(4096) }],
            })
        ).status,
        403,
    );
    // A key for a member without a grant has no reader: accepted, not stored.
    const strangers = await Promise.all(
        Array.from({ length: 25 }, async () => {
            const value = { ...wrapped, member: id() };
            return { wrapped: value, signature: await sign(value, f.who) };
        }),
    );
    assert.equal((await f.request('key-write', { values: strangers })).status, 200);
    assert.equal((await f.request('key-write', { values: [...strangers, strangers[0]] })).status, 400);
    // Permanent fences exist only for real members and registered documents.
    for (let i = 0; i < 5; i++) {
        assert.equal((await f.request('revoke', { target: id() })).status, 200);
        assert.equal((await f.request('delete-document', { document: id() })).status, 200);
    }
    assert.equal(f.kv.values.size, before);
    const link = await liveInvite(f);
    const sealed = { nonce: encode(new Uint8Array(12)), ciphertext: encode(new Uint8Array(96)) };
    const write = async (value: Record<string, unknown>) =>
        (
            await f.request('live-key-write', {
                invitation: link.invitation.id,
                value,
                signature: await sign(value, f.who),
            })
        ).status;
    assert.equal(await write({ document: f.document, epoch: 1, ...sealed }), 200);
    assert.equal(await write({ document: f.document, epoch: 2, ...sealed }), 200);
    assert.equal(await write({ document: f.document, epoch: 900, ...sealed }), 400);
    assert.equal(await write({ document: f.document, epoch: 1, ...sealed, junk: 'J'.repeat(4096) }), 403);
    assert.equal(
        await write({ document: f.document, epoch: 1, nonce: sealed.nonce, ciphertext: encode(new Uint8Array(4096)) }),
        403,
    );
});

test('a pairing request carries only a bounded encrypted device name', async () => {
    const f = await fixture('folder'),
        who = await identity(),
        secret = randomSecret();
    const invitation = {
        v: 1 as const,
        id: id(),
        collection: f.owner.collection,
        document: null,
        mode: 'pair' as const,
        permission: 'edit' as const,
        publicKey: f.who.publicKey,
        participant: f.owner.participant,
        expires: f.kv.time + 600,
    };
    assert.equal(
        (
            await f.request('invite', {
                policy: { invitation, signature: await sign(invitation, f.who) },
                tokenHash: await tokenHash(secret),
            })
        ).status,
        201,
    );
    const candidate = await signGrant(
        {
            ...f.owner,
            member: id(),
            permission: 'edit',
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(randomSecret()),
        },
        who,
    );
    assert.equal(
        (await f.request('pair-request', { invitation: invitation.id, candidate, label: 'L'.repeat(4096) }, secret))
            .status,
        400,
    );
    assert.equal(
        (
            await f.request(
                'pair-request',
                {
                    invitation: invitation.id,
                    candidate,
                    label: { nonce: encode(new Uint8Array(12)), ciphertext: encode(new Uint8Array(4096)) },
                },
                secret,
            )
        ).status,
        400,
    );
    assert.equal(
        (
            await f.request(
                'pair-request',
                {
                    invitation: invitation.id,
                    candidate,
                    label: await sealJson({ name: 'Laptop' }, keyBytes(), { invitation: invitation.id }),
                },
                secret,
            )
        ).status,
        202,
    );
});

test('revoking a Live member frees its slot for the next recipient', async () => {
    const f = await fixture(true),
        link = await liveInvite(f);
    const guests = [];
    for (let i = 0; i < LIMITS.liveGrants; i++) guests.push(await liveJoin(f, link));
    assert.equal(guests.at(-1)!.result.status, 201);
    assert.equal((await liveJoin(f, link)).result.status, 507);
    assert.equal((await f.request('revoke', { target: guests[0].member })).status, 200);
    assert.equal((await liveJoin(f, link)).result.status, 201);
    assert.equal((await f.request('index', { member: guests[0].member }, guests[0].token)).status, 403);
});
