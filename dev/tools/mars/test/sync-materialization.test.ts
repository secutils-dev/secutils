import assert from 'node:assert/strict';
import { test } from 'node:test';
import { indexedDB, IDBKeyRange, IDBObjectStore } from 'fake-indexeddb';
import { materializable, materializationVersion } from '../src/sync/materialization.ts';
import type { ManifestNode, SessionRecord } from '../src/sync/session.ts';
import type { TreeNode } from '../src/tree/model.ts';
import type { SavedUnfinishedSetup, UnfinishedSetupBackup } from '../src/store/db.ts';

Object.assign(globalThis, { indexedDB, IDBKeyRange, location: new URL('https://mars.test/'), BroadcastChannel: undefined });
Object.defineProperty(globalThis, 'navigator', { value: {}, configurable: true });
const db = await import('../src/store/db.ts');
const key = 'a'.repeat(43);
const item = (id: string, parent: string | null, kind: 'doc' | 'folder' = 'folder'): ManifestNode => ({ id, parent, kind, name: id, order: 0, ...(kind === 'doc' ? { key } : {}) });
const registered = (values: ManifestNode[]) => new Map(values.map(value => [value.id, { kind: value.kind, deleted: false }]));
test('only complete folder chains publish, in parent-first order, independent of manifest order', () => {
    const values = [item('doc', 'nested', 'doc'), item('nested', 'folder'), item('folder', 'root')];
    assert.deepEqual(materializable('root', values, registered(values), []).map(item => item.id), ['folder', 'nested', 'doc']);
    assert.deepEqual(materializable('root', values, registered(values), ['folder']), []);
    assert.deepEqual(materializable('root', values, registered(values.slice(0, 2)), []), []);
});
test('cycles, missing/deleted parents, and document parents never fall back to the root', () => {
    const values = [item('a', 'b'), item('b', 'a'), item('orphan', 'missing', 'doc'), item('parent', 'root', 'doc'), item('child', 'parent', 'doc'), item('ok', null, 'doc')];
    assert.deepEqual(materializable('root', values, registered(values), []).map(item => item.id), ['parent', 'ok']);
});

test('a late registration or deletion invalidates the materialized view even when the received manifest is unchanged', () => {
    const directory = { documents: [{ id: 'one', kind: 'folder' as const, creator: 'owner', deleted: false }] };
    const before = materializationVersion('local-root', 'same-manifest', [], directory);
    assert.notEqual(materializationVersion('local-root', 'same-manifest', [], { documents: [...directory.documents, { id: 'two', kind: 'doc', creator: 'owner', deleted: false }] }), before);
    assert.notEqual(materializationVersion('local-root', 'same-manifest', [], { documents: [{ ...directory.documents[0], deleted: true }] }), before);
    assert.notEqual(materializationVersion('replacement-root', 'same-manifest', [], directory), before);
    assert.notEqual(materializationVersion('local-root', 'same-manifest', ['one'], directory), before);
});

async function fixture() {
    const collection = crypto.randomUUID(), member = crypto.randomUUID(), remote = crypto.randomUUID();
    const make = (document: string, kind: 'folder' | 'doc' = 'doc'): TreeNode => ({ id: crypto.randomUUID(), parentId: null, kind, name: document, order: 0, createdAt: 1, updatedAt: 1,
        connection: { collection, member, document, root: remote, kind: 'sync', permission: 'owner', epoch: 1 } });
    const root = make(remote, 'folder');
    const record = (node: TreeNode, projection = '') => ({ revision: 1, binding: node.connection, body: 'manifest-1', projection, outbox: [] } as unknown as SessionRecord);
    await db.commitReplica(root.id, null, record(root), [root]);
    await db.setMeta(`sync:scope:${collection}:${member}`, { collection, member: { grant: { member } }, localRoot: root.id, root: remote, kind: 'folder' });
    return { collection, member, remote, make, root, record };
}
test('two stale tabs publish one remote identity and rebase child parents onto the winning local IDs', async () => {
    const f = await fixture(), folder = crypto.randomUUID(), doc = crypto.randomUUID();
    const plan = () => {
        const parent = f.make(folder, 'folder'), child = f.make(doc);
        return [{ node: parent, parent: f.remote }, { node: child, parent: folder, record: f.record(child, 'Exact body and review bytes') }];
    };
    await Promise.all([db.materializeConnected(f.root, 'manifest-1', plan()), db.materializeConnected(f.root, 'manifest-1', plan())]);
    const nodes = (await db.allNodes()).filter(node => node.connection?.collection === f.collection);
    assert.equal(nodes.length, 3);
    const parent = nodes.find(node => node.connection!.document === folder)!, child = nodes.find(node => node.connection!.document === doc)!;
    assert.equal(child.parentId, parent.id); assert.equal(parent.parentId, f.root.id);
    assert.equal(await db.getText(child.id), 'Exact body and review bytes');
});
test('stale manifests and failed body writes publish no partial hierarchy or replica', async () => {
    const f = await fixture(), child = f.make(crypto.randomUUID());
    await assert.rejects(db.materializeConnected(f.root, 'old-manifest', [{ node: child, parent: f.remote, record: f.record(child) }]), db.ReplicaConflict);
    const invalid = Object.assign(f.record(child), { cannotClone: () => {} });
    await assert.rejects(db.materializeConnected(f.root, 'manifest-1', [{ node: child, parent: f.remote, record: invalid }]));
    assert.equal((await db.allNodes()).some(node => node.id === child.id), false);
    assert.equal(await db.getSide('sync', child.id), undefined);
});
test('a late body commit cannot undo a published hierarchy move or overwrite pending work from another tab', async () => {
    const f = await fixture(), parent = f.make(crypto.randomUUID(), 'folder'), child = f.make(crypto.randomUUID());
    const plan = [{ node: parent, parent: f.remote }, { node: child, parent: parent.connection!.document, record: f.record(child, 'Original') }];
    await db.materializeConnected(f.root, 'manifest-1', plan);
    const stale = { ...child, parentId: f.root.id };
    await db.commitReplica(child.id, 1, { ...f.record(child, 'New local work'), revision: 2 }, [stale], { [child.id]: 'New local work' }, {}, {}, [], child.connection);
    await db.materializeConnected(f.root, 'manifest-1', plan);
    assert.equal((await db.allNodes()).find(node => node.id === child.id)?.parentId, parent.id);
    assert.equal(await db.getText(child.id), 'New local work');
    assert.equal((await db.getSide<SessionRecord>('sync', child.id))?.revision, 2);
});
test('repair archives competing replicas atomically and keeps distinct review text as disconnected local copies', async () => {
    const f = await fixture(), first = f.make(crypto.randomUUID()), extra = { ...f.make('unused'), name: first.name, connection: first.connection }, equal = { ...extra, id: crypto.randomUUID() };
    const sameNameOtherRemote = { ...f.make(crypto.randomUUID()), name: first.name };
    await db.putNodes([first, extra, equal, sameNameOtherRemote], { [first.id]: 'Body', [extra.id]: 'Body plus UNIQUE REVIEW', [equal.id]: 'Body', [sameNameOtherRemote.id]: 'Unrelated' });
    for (const node of [first, extra, equal]) await db.putSide('sync', node.id, f.record(node, node.id === extra.id ? 'UNSENT PROJECTION' : 'Body'));
    // Choose the stable canonical ID explicitly, not the name or thread badge.
    await db.putNodes([{ ...first, createdAt: 0 }]);
    assert.equal(await db.repairDuplicateConnections(), true);
    assert.equal(await db.repairDuplicateConnections(), false);
    const all = await db.allNodes(), active = all.filter(node => node.connection?.document === first.connection!.document);
    assert.equal(active.length, 1); assert.equal(active[0].id, first.id);
    assert.equal(all.find(node => node.id === extra.id)?.connection, undefined);
    assert.equal(await db.getText(extra.id), 'Body plus UNIQUE REVIEW');
    const pending = all.find(node => node.name.endsWith('(pending Sync work)'))!;
    assert.equal(await db.getText(pending.id), 'UNSENT PROJECTION');
    assert.equal(all.find(node => node.id === sameNameOtherRemote.id)?.connection?.document, sameNameOtherRemote.connection!.document);
    const backups = await db.getMeta<UnfinishedSetupBackup[]>(db.unfinishedSetupIndex(f.root.id));
    const saved = await db.getMeta<SavedUnfinishedSetup>(backups![0].key);
    assert.deepEqual(saved?.snapshot.records[extra.id], f.record(extra, 'UNSENT PROJECTION'));
    assert.equal(saved?.snapshot.texts[extra.id], 'Body plus UNIQUE REVIEW');
});
test('repair cannot remove or detach anything if the preservation archive cannot be written', async () => {
    const f = await fixture(), first = f.make(crypto.randomUUID()), extra = { ...first, id: crypto.randomUUID() };
    await db.putNodes([first, extra], { [first.id]: 'Original', [extra.id]: 'Unique review work' });
    for (const node of [first, extra]) await db.putSide('sync', node.id, f.record(node));
    const add = IDBObjectStore.prototype.add;
    IDBObjectStore.prototype.add = function(value, key) {
        if (this.name === 'meta' && String(key).startsWith('sync:unfinished-setup:')) throw new DOMException('Full', 'QuotaExceededError');
        return add.call(this, value, key);
    };
    try { await assert.rejects(db.repairDuplicateConnections(), { name: 'QuotaExceededError' }); }
    finally { IDBObjectStore.prototype.add = add; }
    const all = await db.allNodes();
    assert.ok(all.find(node => node.id === extra.id)?.connection);
    assert.equal(await db.getText(extra.id), 'Unique review work');
    assert.deepEqual(await db.getSide('sync', extra.id), f.record(extra));
    assert.equal(await db.getMeta(db.unfinishedSetupIndex(f.root.id)), undefined);
    await db.repairDuplicateConnections();
});

test('temporary storage uses the same atomic identity and hierarchy rules', async () => {
    db.useTemporaryStorage();
    const f = await fixture(), folder = crypto.randomUUID(), doc = crypto.randomUUID();
    const plan = () => { const parent = f.make(folder, 'folder'), child = f.make(doc); return [{ node: parent, parent: f.remote }, { node: child, parent: folder, record: f.record(child, 'Temporary body') }]; };
    await Promise.all([db.materializeConnected(f.root, 'manifest-1', plan()), db.materializeConnected(f.root, 'manifest-1', plan())]);
    const nodes = await db.allNodes(); assert.equal(nodes.length, 3);
    assert.equal(nodes.find(node => node.connection!.document === doc)?.parentId, nodes.find(node => node.connection!.document === folder)?.id);
});
