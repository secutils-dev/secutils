import assert from 'node:assert/strict';
import { test } from 'node:test';
import { indexedDB, IDBKeyRange } from 'fake-indexeddb';
import type { TreeNode } from '../src/tree/model.ts';

// Keep this test's database isolated from real browser data and all other test files.
Object.assign(globalThis, { indexedDB, IDBKeyRange, location: new URL('https://mars.test/'), BroadcastChannel: undefined });
Object.defineProperty(globalThis, 'navigator', { value: {}, configurable: true });
const db = await import('../src/store/db.ts');
const node = (id: string): TreeNode => ({ id, kind: 'doc', parentId: null, name: 'Local', order: 0, createdAt: 1, updatedAt: 1 });

test('projection, replica, pending envelope, bindings, and progress commit atomically', async () => {
    const id = crypto.randomUUID(), initial = { revision: 1, body: new Uint8Array([1, 2]), outbox: [{ operation: 'one', envelope: null }], progress: {} };
    await db.commitReplica(id, null, initial, [node(id)], { [id]: 'locally saved' }, { [`sync:binding:${id}`]: { remote: 'remote-one' } });
    assert.equal(await db.getText(id), 'locally saved');
    assert.deepEqual(await db.getSide('sync', id), initial);
    assert.deepEqual(await db.getMeta(`sync:binding:${id}`), { remote: 'remote-one' });
    const received = { ...initial, revision: 2, progress: { peer: 2 } };
    await db.commitReplica(id, 1, received, [node(id)], { [id]: 'local and remote' });
    assert.equal(await db.getText(id), 'local and remote');
    assert.deepEqual(await db.getSide('sync', id), received);
});

test('stale tabs cannot overwrite a newer replica or advance an unsaved cursor', async () => {
    const id = crypto.randomUUID();
    await db.commitReplica(id, null, { revision: 1, cursor: 1 }, [node(id)], { [id]: 'first' });
    const settled = await Promise.allSettled([
        db.commitReplica(id, 1, { revision: 2, cursor: 2 }, [], { [id]: 'winner A' }),
        db.commitReplica(id, 1, { revision: 2, cursor: 3 }, [], { [id]: 'winner B' }),
    ]);
    assert.equal(settled.filter((result) => result.status === 'fulfilled').length, 1);
    assert.equal(settled.filter((result) => result.status === 'rejected').length, 1);
    const stored = await db.getSide<{ revision: number; cursor: number }>('sync', id);
    assert.equal(await db.getText(id), stored?.cursor === 2 ? 'winner A' : 'winner B');
    await assert.rejects(db.commitReplica(id, 1, { revision: 3, cursor: 99 }, [], { [id]: 'lost edit' }), db.ReplicaConflict);
    assert.equal((await db.getSide<{ cursor: number }>('sync', id))?.cursor, stored?.cursor);
});

test('a structured-clone storage failure aborts projection and cursor together', async () => {
    const id = crypto.randomUUID();
    await db.commitReplica(id, null, { revision: 1, cursor: 1 }, [node(id)], { [id]: 'keep this' });
    await assert.rejects(db.commitReplica(id, 1, { revision: 2, cursor: 999, invalid: () => {} }, [], { [id]: 'do not commit' }));
    assert.equal(await db.getText(id), 'keep this');
    assert.deepEqual(await db.getSide('sync', id), { revision: 1, cursor: 1 });
});

test('node deletion deletes its connected side record, without deleting another replica', async () => {
    const one = crypto.randomUUID(), two = crypto.randomUUID();
    for (const id of [one, two]) await db.commitReplica(id, null, { revision: 1 }, [node(id)], { [id]: id });
    await db.deleteNodes([one]);
    assert.equal(await db.getSide('sync', one), undefined);
    assert.deepEqual(await db.getSide('sync', two), { revision: 1 });
});

test('sync persistence preserves version 2 databases and unrelated extra stores', async () => {
    const upgraded = await new Promise<IDBDatabase>((resolve, reject) => {
        const open = indexedDB.open('mars', 2);
        open.onupgradeneeded = () => { open.result.createObjectStore('legacy-extra').put('retain', 'marker'); };
        open.onsuccess = () => resolve(open.result); open.onerror = () => reject(open.error);
    });
    upgraded.close();
    const id = crypto.randomUUID();
    await db.commitReplica(id, null, { revision: 1 }, [node(id)], { [id]: 'version two' });
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
        const open = indexedDB.open('mars'); open.onsuccess = () => resolve(open.result); open.onerror = () => reject(open.error);
    });
    assert.equal(database.version, 2);
    const marker = await new Promise((resolve, reject) => {
        const request = database.transaction('legacy-extra').objectStore('legacy-extra').get('marker');
        request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error);
    });
    assert.equal(marker, 'retain'); database.close();
});

test('recovery restores its entire replica set atomically and never overwrites an existing connection', async () => {
    const one = node(crypto.randomUUID()), two = node(crypto.randomUUID()), collection = crypto.randomUUID(), member = crypto.randomUUID();
    one.connection = { collection, member, document: crypto.randomUUID(), permission: 'owner', epoch: 1, kind: 'sync' };
    two.connection = { ...one.connection, document: crypto.randomUUID() };
    await assert.rejects(db.restoreConnected([one, two], { [one.id]: { revision: 1, projection: 'must roll back' }, [two.id]: { revision: 1, projection: 'invalid', invalid: () => {} } } as Record<string, { revision: number; projection: string }>, { recoveryMarker: 'must roll back' }));
    assert.equal((await db.allNodes()).some((item) => item.id === one.id), false);
    assert.equal(await db.getText(one.id), undefined);
    assert.equal(await db.getMeta('recoveryMarker'), undefined);
    await db.restoreConnected([one, two], { [one.id]: { revision: 1, projection: 'one' }, [two.id]: { revision: 1, projection: 'two' } }, { recoveryMarker: 'restored' });
    assert.equal(await db.getText(one.id), 'one');
    const duplicate = { ...one, id: crypto.randomUUID() };
    await assert.rejects(db.restoreConnected([duplicate], { [duplicate.id]: { revision: 1, projection: 'do not replace' } }, { recoveryMarker: 'do not replace' }), db.ReplicaConflict);
    assert.equal(await db.getMeta('recoveryMarker'), 'restored');
    assert.equal(await db.getText(one.id), 'one');
});

test('confirmed matches retain local IDs, keep backups, and reject changes made during the dialog', async () => {
    const local = node(crypto.randomUUID()), incoming = node(crypto.randomUUID()), backup = node(crypto.randomUUID());
    incoming.connection = { collection: crypto.randomUUID(), member: crypto.randomUUID(), document: crypto.randomUUID(), permission: 'edit', epoch: 1, kind: 'sync' };
    const record = { revision: 1, projection: 'connected text' };
    await db.putNodes([local], { [local.id]: 'local text' });
    await db.commitReplica(incoming.id, null, record, [incoming], { [incoming.id]: record.projection });
    const connected = { ...local, connection: incoming.connection, pending: true as const };
    await db.putNodes([{ ...local, name: 'Changed in another tab' }]);
    await assert.rejects(db.adoptConnected(incoming.id, local, connected, 'local text', record, {}, backup), db.ReplicaConflict);
    assert.equal(await db.getText(local.id), 'local text');
    assert.equal((await db.allNodes()).some((node) => node.id === incoming.id), true);
    await db.putNodes([local]);
    await db.adoptConnected(incoming.id, local, connected, 'local text', record, { matchMarker: 'connected' }, backup);
    assert.equal(await db.getText(local.id), 'connected text');
    assert.equal(await db.getText(backup.id), 'local text');
    assert.equal(await db.getSide('base', local.id), 'local text');
    assert.equal(await db.getSide('sync', incoming.id), undefined);
    assert.equal((await db.allNodes()).some((node) => node.id === incoming.id), false);
});

test('temporary live promotion reopens storage without replacing the same-profile owner credentials', async () => {
    const owner = node(crypto.randomUUID()), guest = node(crypto.randomUUID()), collection = crypto.randomUUID();
    const ownerScope = `sync:scope:${collection}:${crypto.randomUUID()}`, guestScope = `sync:scope:${collection}:${crypto.randomUUID()}`;
    await db.putNodes([owner], { [owner.id]: 'owner private text' });
    await db.setMeta(ownerScope, { token: 'owner credential', root: owner.id });
    const ownerVector = { id: owner.id, at: 10, chunks: [new Float32Array([1, 2])] }, guestVector = { id: guest.id, at: 20, chunks: [new Float32Array([3, 4])] };
    await db.putSide('vec', owner.id, ownerVector);
    const originalGeneration = db.storageGeneration();
    db.useTemporaryStorage();
    assert.equal(db.storageGeneration(), originalGeneration + 1);
    await db.putSide('vec', guest.id, guestVector);
    await db.promoteConnected(guest, 'saved guest text', { revision: 1 }, { [guestScope]: { token: 'document-only guest credential' } });
    assert.equal(db.isTemporaryStorage(), false);
    assert.equal(db.storageGeneration(), originalGeneration + 2);
    assert.deepEqual(await db.getSide('vec', owner.id), ownerVector);
    assert.deepEqual(await db.getSide('vec', guest.id), guestVector);
    assert.equal(await db.getText(owner.id), 'owner private text');
    assert.equal(await db.getText(guest.id), 'saved guest text');
    assert.deepEqual(await db.getMeta(ownerScope), { token: 'owner credential', root: owner.id });
    assert.deepEqual(await db.getMeta(guestScope), { token: 'document-only guest credential' });
});

test('rotation metadata commits compare revisions and clear with revocation history atomically', async () => {
    const key = `rotation-test:${crypto.randomUUID()}`;
    await db.commitMeta(key, null, { revision: 1, key: 'preserve' });
    await assert.rejects(db.commitMeta(key, null, { revision: 1, key: 'overwrite' }), db.ReplicaConflict);
    await db.commitMeta(key, 1, { revision: 2, key: 'preserve' });
    await assert.rejects(db.commitMeta(key, 1, null, { [`${key}:history`]: 'must not commit' }), db.ReplicaConflict);
    assert.equal(await db.getMeta(`${key}:history`), undefined);
    await db.commitMeta(key, 2, null, { [`${key}:history`]: ['revoked invitation'] });
    assert.equal(await db.getMeta(key), null);
    assert.deepEqual(await db.getMeta(`${key}:history`), ['revoked invitation']);
});

test('recovery capture includes only the selected member’s replicas and a consistent rotation journal', async () => {
    const collection = crypto.randomUUID(), member = crypto.randomUUID(), one = node(crypto.randomUUID()), other = node(crypto.randomUUID());
    one.connection = { collection, member, document: crypto.randomUUID(), permission: 'owner', kind: 'sync', epoch: 1 };
    other.connection = { ...one.connection, member: crypto.randomUUID() };
    await db.commitReplica(one.id, null, { revision: 1, marker: 'selected' }, [one]);
    await db.commitReplica(other.id, null, { revision: 1, marker: 'private other member' }, [other]);
    const key = `capture:${crypto.randomUUID()}`; await db.setMeta(key, { revision: 1 });
    const value = await db.captureConnected(collection, member, [key]);
    assert.deepEqual(value.nodes.map((node) => node.id), [one.id]);
    assert.deepEqual(value.records, [{ revision: 1, marker: 'selected' }]);
    assert.deepEqual(value.metadata[key], { revision: 1 });
    const excluded = node(crypto.randomUUID()); excluded.connection = { ...one.connection, document: crypto.randomUUID() };
    await db.commitReplica(excluded.id, null, { revision: 1, marker: 'outside the requested export' }, [excluded]);
    const subset = await db.captureConnected(collection, member, [key], [one.id]);
    assert.deepEqual(subset.nodes.map(node => node.id), [one.id]);
    assert.deepEqual(subset.records, [{ revision: 1, marker: 'selected' }]);
    assert.deepEqual(subset.excludedDocuments, [excluded.connection.document]);
});
