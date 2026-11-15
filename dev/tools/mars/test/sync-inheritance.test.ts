import assert from 'node:assert/strict';
import { test } from 'node:test';
import { insideSyncedFolder, legacyLocalCopy, localCopyPlan } from '../src/sync/inheritance.ts';
import type { TreeNode } from '../src/tree/model.ts';

const node = (id: string, parentId: string | null, kind: TreeNode['kind'] = 'doc'): TreeNode => ({
    id,
    parentId,
    kind,
    name: id,
    order: 0,
    createdAt: 1,
    updatedAt: 1,
    originId: id,
});
const root = node('root', null, 'folder');
root.connection = {
    collection: 'collection',
    member: 'member',
    document: 'remote-root',
    root: 'remote-root',
    kind: 'sync',
    permission: 'owner',
    epoch: 1,
};

test('root detachment leaves its whole subtree local; partial detachment must leave the active ancestor', () => {
    const folder = node('folder', root.id, 'folder'),
        doc = node('doc', folder.id),
        all = [root, folder, doc];
    assert.equal(insideSyncedFolder(all, doc.id), true);
    assert.equal(insideSyncedFolder(all, doc.id, new Set([root.id, folder.id, doc.id])), false);
    assert.equal(insideSyncedFolder(all, folder.id, new Set([folder.id, doc.id])), true);
});

test('local-copy relocation preserves IDs, complete subtrees, content metadata, and foreign bindings', () => {
    const folder = node('folder', root.id, 'folder'),
        doc = node('doc', folder.id);
    folder.connection = { ...root.connection!, document: 'foreign-root', collection: 'foreign' };
    const all = [root, folder, doc];
    const plan = localCopyPlan(all, new Set([folder.id, doc.id]));
    assert.equal(plan.length, 2);
    assert.equal(plan[0].parentId, null);
    assert.equal(plan[1].id, folder.id);
    assert.deepEqual(plan[1].connection, folder.connection);
    assert.equal(doc.parentId, folder.id);
    assert.equal(folder.parentId, root.id);
});

test('legacy explicit disconnections are not mistaken for new imports', () => {
    const fresh = node('fresh', root.id),
        detached = { ...fresh, originId: 'fresh-independent-origin' };
    assert.equal(legacyLocalCopy(fresh, true), false);
    assert.equal(legacyLocalCopy(detached, true), true);
    assert.equal(legacyLocalCopy(detached, false), false);
    assert.equal(legacyLocalCopy({ ...fresh, originId: undefined }, false), true);
});
