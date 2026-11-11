import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DurableBody, type DurableRecord } from '../src/sync/durable.ts';
import { identity, keyBytes, checkpointDigest } from '../src/sync/crypto.ts';
import { BodyReplica } from '../src/sync/replica.ts';
import { checkpointKey, reverseSequence, type SyncBinding } from '../src/sync/protocol.ts';
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
