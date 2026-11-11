import assert from 'node:assert/strict';
import { test } from 'node:test';
import { incomingFolder } from '../src/io/destination.ts';
import type { TreeNode } from '../src/tree/model.ts';

const folder = (id: string, parentId: string | null = null): TreeNode => ({
    id,
    parentId,
    kind: 'folder',
    name: id,
    order: 0,
    createdAt: 1,
    updatedAt: 1,
});
test('a destination is a stable local folder ID, not a name or the last selected document', () => {
    const selected = folder('selected'),
        sameName = { ...folder('other'), name: selected.name };
    assert.equal(incomingFolder([selected, sameName], selected.id), selected);
    selected.name = 'Renamed';
    assert.equal(incomingFolder([selected], selected.id)?.name, 'Renamed');
    assert.equal(incomingFolder([sameName], selected.id), undefined);
    assert.equal(incomingFolder([{ ...selected, kind: 'doc' }], selected.id), undefined);
});
test('only editable active connections can receive new documents, including through a local child folder', () => {
    const root = folder('root'),
        child = folder('child', root.id);
    root.connection = {
        collection: 'collection',
        member: 'member',
        document: root.id,
        kind: 'sync',
        permission: 'owner',
        epoch: 1,
    };
    assert.equal(incomingFolder([root, child], child.id), child);
    root.connection.permission = 'view';
    assert.equal(incomingFolder([root, child], child.id), undefined);
    root.connection.permission = 'comment';
    assert.equal(incomingFolder([root, child], child.id), undefined);
    root.connection.permission = 'edit';
    assert.equal(incomingFolder([root, child], child.id), child);
    root.connection.phase = 'ended';
    assert.equal(incomingFolder([root, child], child.id), undefined);
});
