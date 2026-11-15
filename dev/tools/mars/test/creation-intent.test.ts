import assert from 'node:assert/strict';
import { test } from 'node:test';
import { creationIntent } from '../src/sync/creation.ts';
import type { TreeNode } from '../src/tree/model.ts';

const node = (kind: TreeNode['kind'], parentId: string | null): TreeNode => ({
    id: crypto.randomUUID(),
    kind,
    parentId,
    name: 'Local',
    order: 0,
    createdAt: 1,
    updatedAt: 1,
});
test('explicit creations under a pending child retain their original folder scope and carry no keys in tree metadata', () => {
    const root = node('folder', null);
    root.connection = {
        collection: crypto.randomUUID(),
        member: crypto.randomUUID(),
        document: crypto.randomUUID(),
        epoch: 1,
        permission: 'owner',
        kind: 'sync',
    };
    root.connection.root = root.connection.document;
    const folder = node('folder', root.id),
        doc = node('doc', folder.id),
        all = [root, folder, doc];
    const a = creationIntent(all, folder)!,
        b = creationIntent(all, doc)!;
    assert.equal(a.collection, b.collection);
    assert.equal(a.root, b.root);
    assert.notEqual(a.document, b.document);
    assert.equal(b.key.length, 43);
    assert.equal('key' in doc.syncPending!, false);
});
test('new local roots, read-only folders, ended connections, and live folders cannot implicitly enroll documents', () => {
    const folder = node('folder', null),
        doc = node('doc', folder.id);
    assert.equal(creationIntent([folder, doc], doc), undefined);
    for (const permission of ['view', 'comment'] as const) {
        folder.connection = {
            collection: crypto.randomUUID(),
            member: crypto.randomUUID(),
            document: crypto.randomUUID(),
            epoch: 1,
            permission,
            kind: 'sync',
        };
        assert.equal(creationIntent([folder, doc], doc), undefined);
    }
    folder.connection!.permission = 'owner';
    folder.connection!.phase = 'ended';
    assert.equal(creationIntent([folder, doc], doc), undefined);
    folder.connection!.phase = 'current';
    folder.connection!.kind = 'live';
    assert.equal(creationIntent([folder, doc], doc), undefined);
});
