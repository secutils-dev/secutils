import assert from 'node:assert/strict';
import { test } from 'node:test';
import { indexedDB, IDBKeyRange } from 'fake-indexeddb';
import type { TreeNode } from '../src/tree/model.ts';

Object.assign(globalThis, {
    indexedDB,
    IDBKeyRange,
    location: new URL('https://mars.test/'),
    BroadcastChannel: undefined,
});
Object.defineProperty(globalThis, 'navigator', { value: {}, configurable: true });
const db = await import('../src/store/db.ts');

// Both backends enforce the same transaction contract. Temporary mode is selected only after library checks.
for (const temporary of [false, true]) {
    test(`connected imports commit source, body, baseline, and outbox atomically (${temporary ? 'temporary' : 'IndexedDB'})`, async () => {
        if (temporary) db.useTemporaryStorage();
        const node: TreeNode = {
            id: crypto.randomUUID(),
            kind: 'doc',
            name: 'Retained',
            parentId: null,
            order: 0,
            createdAt: 1,
            updatedAt: 1,
            opened: true,
            connection: {
                collection: 'collection',
                document: crypto.randomUUID(),
                member: 'member',
                permission: 'owner',
                epoch: 1,
                kind: 'sync',
            },
        };
        const binding = node.connection!;
        await db.putNodes([node], { [node.id]: 'Original' }, { extras: { [`opened:${node.id}`]: 'Original' } });
        await db.commitReplica(node.id, null, { revision: 1, binding, outbox: [] }, [node]);
        const expected = { ...(await db.getDocument(node.id))!, opened: 'Original' };
        const update: TreeNode = {
            ...expected.node,
            pending: true,
            importSource: { destinationId: null, path: ['Full', 'Source.md'] },
            updatedAt: 2,
        };
        const next = { revision: 2, binding, outbox: [{ operation: 'pending' }] };
        await assert.rejects(
            db.commitReplica(
                node.id,
                1,
                next,
                [update],
                { [node.id]: 'Incoming' },
                {},
                { [`opened:${node.id}`]: 'Incoming', [`base:${node.id}`]: () => {} },
                [],
                binding,
                expected,
            ),
        );
        assert.deepEqual(await db.getDocument(node.id), { node: expected.node, text: 'Original' });
        assert.equal((await db.getSide<{ revision: number }>('sync', node.id))!.revision, 1);
        assert.equal(await db.getSide('opened', node.id), 'Original');
        assert.equal(await db.getSide('base', node.id), undefined);
        await db.putSide('opened', node.id, 'Other source');
        await assert.rejects(
            db.commitReplica(
                node.id,
                1,
                next,
                [update],
                { [node.id]: 'Incoming' },
                {},
                { [`opened:${node.id}`]: 'Incoming' },
                [],
                binding,
                expected,
            ),
            db.ReplicaConflict,
        );
        assert.equal((await db.getSide<{ revision: number }>('sync', node.id))!.revision, 1);
        await db.putSide('opened', node.id, 'Original');
        await db.commitReplica(
            node.id,
            1,
            next,
            [update],
            { [node.id]: 'Incoming' },
            {},
            { [`opened:${node.id}`]: 'Incoming', [`base:${node.id}`]: 'Original' },
            [],
            binding,
            expected,
        );
        const after = (await db.getDocument(node.id))!;
        assert.equal(after.text, 'Incoming');
        assert.deepEqual(after.node.importSource, update.importSource);
        assert.equal(await db.getSide('opened', node.id), 'Incoming');
        assert.equal(await db.getSide('base', node.id), 'Original');
        assert.deepEqual(await db.getSide('sync', node.id), next);
        // A delayed acknowledgement cannot erase a browser-local source identity with its stale node.
        await db.commitReplica(node.id, 2, { ...next, revision: 3 }, [node], {}, {}, {}, [], binding);
        assert.deepEqual((await db.getDocument(node.id))!.node.importSource, update.importSource);
    });
}
