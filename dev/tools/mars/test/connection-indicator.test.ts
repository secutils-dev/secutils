import assert from 'node:assert/strict';
import { test } from 'node:test';
import { connectionBadgeVisible, hasSyncGlyph, syncedAncestor } from '../src/ui/connection-indicator.ts';
import type { TreeNode } from '../src/tree/model.ts';

const root: TreeNode = {
    id: 'root',
    name: 'Personal',
    parentId: null,
    kind: 'folder',
    order: 0,
    createdAt: 1,
    updatedAt: 1,
    connection: {
        collection: 'collection',
        member: 'member',
        document: 'remote-root',
        root: 'remote-root',
        kind: 'sync',
        epoch: 1,
        permission: 'owner',
        phase: 'current',
    },
};
const folder: TreeNode = {
    ...root,
    id: 'folder',
    parentId: root.id,
    connection: { ...root.connection!, document: 'remote-folder' },
};
const doc: TreeNode = {
    ...folder,
    id: 'doc',
    kind: 'doc',
    parentId: folder.id,
    connection: { ...root.connection!, document: 'remote-doc' },
};

test('healthy inherited rows use their embedded mark, while roots retain the activity control', () => {
    const nodes = [root, folder, doc];
    assert.equal(connectionBadgeVisible(nodes, root), true);
    for (const row of [folder, doc]) {
        assert.equal(connectionBadgeVisible(nodes, row), false);
        assert.equal(syncedAncestor(nodes, row)?.id, root.id);
        assert.equal(hasSyncGlyph(row), true);
    }
    const offline = { ...doc, connection: { ...doc.connection!, phase: 'offline' as const } };
    assert.equal(connectionBadgeVisible(nodes, offline), false);
});

test('enrollment issues and Live links remain explicit, with no misleading mark on ended content', () => {
    const nodes = [root, folder, doc];
    const queued = {
        ...doc,
        syncPending: { collection: 'collection', member: 'member', root: 'remote-root', phase: 'attention' as const },
    };
    assert.equal(connectionBadgeVisible(nodes, queued), true);
    assert.equal(connectionBadgeVisible(nodes, { ...doc, connection: { ...doc.connection!, liveShared: true } }), true);
    assert.equal(connectionBadgeVisible(nodes, { ...doc, connection: { ...doc.connection!, kind: 'live' } }), true);
    const ended = { ...doc, connection: { ...doc.connection!, phase: 'ended' as const } };
    assert.equal(connectionBadgeVisible(nodes, ended), true);
    assert.equal(hasSyncGlyph(ended), false);
    assert.equal(
        connectionBadgeVisible(
            [{ ...root, connection: { ...root.connection!, phase: 'ended' } }, folder, ended],
            ended,
        ),
        false,
    );
});
