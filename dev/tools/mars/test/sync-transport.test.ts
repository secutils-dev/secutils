import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DurableBody, type DurableRecord } from '../src/sync/durable.ts';
import { identity, keyBytes, checkpointDigest, randomSecret, sealCheckpoint, signGrant, tokenHash, verifyGrant } from '../src/sync/crypto.ts';
import { BodyReplica } from '../src/sync/replica.ts';
import { checkpointKey, decode, jsonBytes, reverseSequence, type SyncBinding } from '../src/sync/protocol.ts';
import { Session } from '../src/sync/session.ts';
import { Transport, type ConnectionStatus } from '../src/sync/transport.ts';

async function replica(owner = false) {
    const who = await identity(), key = keyBytes(), binding: SyncBinding = { collection: crypto.randomUUID(), document: crypto.randomUUID(), member: crypto.randomUUID(),
        permission: owner ? 'owner' : 'view', kind: 'sync', epoch: 1 };
    let record: DurableRecord | undefined;
    const repository = { get: async () => record ? structuredClone(record) : undefined,
        commit: async (expected: number | null, next: DurableRecord) => { assert.equal(record?.revision ?? null, expected); record = structuredClone(next); } };
    const seed = new BodyReplica(); seed.seed('text');
    const client = owner ? await DurableBody.create(binding, 'text', repository, who, key) : await DurableBody.join(binding, seed.checkpoint(), repository, who, key);
    seed.destroy(); return { who, key, client, repository };
}
const deferred = <T>() => {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>((r) => { resolve = r; });
    return { promise, resolve };
};
const response = (value: unknown) => new Response(JSON.stringify(value), { status: 200, headers: { 'Content-Type': 'application/json' } });

test('checkpoint keys expose durable cursors only after strict namespace and sequence validation', () => {
    const collection = crypto.randomUUID(), document = crypto.randomUUID(), member = crypto.randomUUID(), operation = crypto.randomUUID();
    const key = `sync/v1/c/${collection}/updates/${document}/${member}/body/${reverseSequence(12)}.${operation}`;
    assert.deepEqual(checkpointKey(key), { collection, document, member, stream: 'body', sequence: 12 });
    for (const invalid of [key.replace('sync/v1/', 'other/v1/'), key.replace('/body/', '/invalid/'), key + '/extra', key.replace(reverseSequence(12), reverseSequence(0)), key.replace(operation, 'invalid')]) assert.equal(checkpointKey(invalid), null);
});

test('warm polling skips applied checkpoints before GET and requests directory metadata only on the first index page', async () => {
    const f = await replica(), visibility = new EventTarget(), connectivity = new EventTarget(), finished = deferred<void>();
    const calls: { action: string; values: Record<string, unknown> }[] = [], directory = { documents: [{ id: f.client.binding.document, kind: 'folder', creator: f.client.binding.member, deleted: false }] };
    let pages = 0, known = 0;
    const transport = new Transport({ origin: 'https://mars.test', token: 'private token', ownerPublicKey: f.who.publicKey, replica: f.client, collectionScope: true, directory: true,
        environment: { visible: () => true, online: () => true, visibility, connectivity }, status: () => {}, projection: () => {},
        known: async () => { known++; return true; },
        fetch: async (url, options) => {
            const action = new URL(String(url)).pathname.split('/').at(-1)!, values = JSON.parse(options!.body as string); calls.push({ action, values });
            assert.equal(action, 'index'); pages++;
            return response({ entries: [{ key: `document/member/body/checkpoint-${pages}` }], complete: pages === 2, cursor: 'next', ...(pages === 1 ? { directory } : {}) });
        }, cycle: async (_signal, received) => { assert.deepEqual(received, directory); finished.resolve(); } });
    try { transport.start(); await finished.promise; assert.equal(known, 2); assert.equal(calls.length, 2); assert.equal(calls[0].values.directory, true); assert.equal(calls[1].values.directory, undefined); }
    finally { transport.dispose(); }
});

test('hidden tabs abort reads, visible tabs resume, and pagehide aborts even before document.hidden changes', async () => {
    const f = await replica(), visibility = new EventTarget(), connectivity = new EventTarget();
    let visible = true, requests = 0;
    const first = deferred<void>(), second = deferred<void>(), aborted = deferred<void>();
    const mockFetch: typeof fetch = async (_url, options) => {
        requests++;
        if (requests > 1) { second.resolve(); return response({ entries: [], complete: true, cursor: null }); }
        first.resolve();
        return new Promise((_resolve, reject) => options?.signal?.addEventListener('abort', () => { aborted.resolve(); reject(new DOMException('Aborted', 'AbortError')); }, { once: true }));
    };
    const transport = new Transport({ origin: 'https://mars.test', token: 'private token', ownerPublicKey: f.who.publicKey, replica: f.client, fetch: mockFetch,
        environment: { visible: () => visible, online: () => true, visibility, connectivity }, status: () => {}, projection: () => {} });
    try {
        transport.start(); await first.promise;
        visible = false; visibility.dispatchEvent(new Event('visibilitychange')); await aborted.promise;
        assert.equal(requests, 1);
        visible = true; visibility.dispatchEvent(new Event('visibilitychange')); await second.promise;
        connectivity.dispatchEvent(new Event('pagehide'));
    } finally { transport.dispose(); }
});

test('an already-started upload finishes when hidden, but later pending work remains local', async () => {
    const f = await replica(true), visibility = new EventTarget(), connectivity = new EventTarget();
    let visible = true;
    const started = deferred<void>(), reply = deferred<Response>(), acknowledged = deferred<void>();
    const states: ConnectionStatus[] = [];
    let captured: { url: string; options: RequestInit; envelope: unknown } | undefined;
    const transport = new Transport({ origin: 'https://mars.test', token: 'header-only-secret', ownerPublicKey: f.who.publicKey, replica: f.client,
        environment: { visible: () => visible, online: () => true, visibility, connectivity },
        fetch: async (url, options) => {
            assert.match(String(url), /\/sync\/append$/);
            captured = { url: String(url), options: options!, envelope: JSON.parse(options!.body as string).envelope };
            started.resolve(); return reply.promise;
        }, status: (state) => { states.push(state); if (state === 'waiting') acknowledged.resolve(); }, projection: () => {} });
    try {
        transport.start(); await started.promise;
        assert.ok(captured);
        assert.equal(captured.options.signal, undefined);
        assert.equal(captured.options.credentials, 'omit');
        assert.equal(captured.url.includes('header-only-secret'), false);
        assert.equal((captured.options.body as string).includes('header-only-secret'), false);
        visible = false; visibility.dispatchEvent(new Event('visibilitychange'));
        await f.client.edit({ from: 0, to: 0, insert: 'still local ' });
        const envelope = captured.envelope as { checkpoint: { operation: string } };
        reply.resolve(response({ operation: envelope.checkpoint.operation, digest: await checkpointDigest(envelope) }));
        await acknowledged.promise;
        assert.equal(await f.client.pending(), 1);
        assert.equal((await f.repository.get())?.projection, 'still local text');
        assert.equal(states.at(-1), 'waiting');
    } finally { transport.dispose(); }
});

for (const variant of ['undecryptable content', 'an operation signed for another actor'] as const) {
    test(`a checkpoint with ${variant} is skipped, and the connection continues with the next checkpoint`, async () => {
        const who = await identity(), token = randomSecret(), collection = crypto.randomUUID(), document = crypto.randomUUID();
        const owner = await signGrant({ v: 1, collection, member: crypto.randomUUID(), participant: crypto.randomUUID(), permission: 'owner', document: null,
            publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(token), expires: 0 }, who);
        const record = Session.seed({ collection, document, member: owner.grant.member, permission: 'owner', kind: 'sync', epoch: 1 }, '# Doc\n\nBody.', who, token, randomSecret(), owner, 'Document');
        let stored = structuredClone(record);
        const session = new Session(record, { get: async () => structuredClone(stored), commit: async (_expected, next) => { stored = structuredClone(next); } });
        // A comment-level participant uploads one checkpoint that no receiver can verify.
        const guest = await identity(), guestId = crypto.randomUUID();
        const guestGrant = await signGrant({ ...owner.grant, member: guestId, participant: guestId, permission: 'comment', document, publicKey: guest.publicKey, boxKey: guest.boxKey }, who);
        const key = variant === 'undecryptable content' ? keyBytes() : decode(record.key);
        const body = jsonBytes({ operations: [{ operation: { actor: owner.grant.member }, signature: '' }] });
        const rejected = await sealCheckpoint({ v: 1, collection, document, member: guestId, stream: 'review', epoch: 1, sequence: 1, operation: crypto.randomUUID() }, body, key, guest);
        const valid = await sealCheckpoint({ v: 1, collection, document, member: guestId, stream: 'review', epoch: 1, sequence: 2, operation: crypto.randomUUID() }, jsonBytes({ operations: [] }), decode(record.key), guest);
        const keyOf = (envelope: typeof rejected) => `sync/v1/c/${collection}/updates/${document}/${guestId}/review/${reverseSequence(envelope.checkpoint.sequence)}.${envelope.checkpoint.operation}`;
        const statuses: string[] = [], received: number[] = [], finished = deferred<void>();
        const transport = new Transport({ origin: 'https://mars.test', token, ownerPublicKey: who.publicKey, streams: ['body', 'review', 'tree'],
            replica: { binding: record.binding, prepare: async () => null, pending: async () => 0, acknowledge: async () => {}, receive: async () => null },
            environment: { visible: () => true, online: () => true, visibility: new EventTarget(), connectivity: new EventTarget() },
            status: (state: ConnectionStatus) => { statuses.push(state); }, projection: () => {},
            authorValid: (author) => verifyGrant(author, owner.grant),
            receive: async (envelope, author) => { const text = await session.receive(envelope, author); received.push(envelope.checkpoint.sequence); return text; },
            cycle: async () => { finished.resolve(); },
            fetch: async (url, options) => {
                const action = new URL(String(url)).pathname.split('/').at(-1)!, values = JSON.parse(options!.body as string);
                // The index lists the unverifiable checkpoint first, like a newest-first listing does.
                if (action === 'index') return response({ entries: [{ key: keyOf(rejected) }, { key: keyOf(valid) }], complete: true });
                return response({ envelope: values.key === keyOf(rejected) ? rejected : valid, author: guestGrant });
            } });
        try { transport.start(); await finished.promise; }
        finally { transport.dispose(); }
        assert.deepEqual(received, [2]);
        assert.equal(stored.progress[`${guestId}/review`], 2);
        assert.ok(!statuses.includes('ended') && !statuses.includes('attention'), statuses.join(','));
    });
}
