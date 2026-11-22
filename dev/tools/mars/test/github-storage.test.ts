import assert from 'node:assert/strict';
import { test } from 'node:test';
import { indexedDB, IDBKeyRange } from 'fake-indexeddb';
import type { TreeNode } from '../src/tree/model.ts';
import type { StructuralJob } from '../src/github/structure.ts';

Object.assign(globalThis, {
    indexedDB,
    IDBKeyRange,
    location: new URL('https://mars.test/'),
    BroadcastChannel: undefined,
});
Object.defineProperty(globalThis, 'navigator', { value: {}, configurable: true });
const db = await import('../src/store/db.ts');
const { gitHubCredentials, removeGitHubCredential, saveGitHubCredential, withGitHubCredential } =
    await import('../src/github/credential-store.ts');

test('reusable GitHub credentials keep tokens out of labels and use the latest sealed secret', async () => {
    const first = await saveGitHubCredential({ name: 'Docs read-only', owner: 'Example' }, 'synthetic-first');
    const second = await saveGitHubCredential({ name: 'Docs write', owner: 'Example' }, 'synthetic-second');
    assert.notEqual(first, second);
    const labels = await gitHubCredentials();
    assert.deepEqual(labels, [
        { id: first, name: 'Docs read-only', owner: 'Example' },
        { id: second, name: 'Docs write', owner: 'Example' },
    ]);
    const read = (id: string) => withGitHubCredential(id, async (auth) => auth);
    const before = await read(first);
    assert.equal(before.token, 'synthetic-first');
    await saveGitHubCredential({ id: first, name: 'Documentation', owner: 'Example' }, '');
    assert.deepEqual(await read(first), before);
    await saveGitHubCredential({ id: first, name: 'Documentation', owner: 'Example' }, 'synthetic-rotated');
    const after = await read(first);
    assert.equal(after.token, 'synthetic-rotated');
    assert.notEqual(after.scope, before.scope);
    assert.equal((await read(second)).token, 'synthetic-second');
    assert.doesNotMatch(JSON.stringify(await db.getSide('github-credential', first)), /synthetic/);
    await removeGitHubCredential(first);
    await assert.rejects(read(first), /no longer available/);
    await assert.rejects(saveGitHubCredential({ id: first, name: 'Documentation', owner: '' }, 'new'), /removed/);
    await removeGitHubCredential(second);
});

test('rules survive stale metadata saves and invalidate a prepared pull without discarding edited files', async () => {
    const root: TreeNode = {
        id: 'filter-root',
        name: 'Repository',
        kind: 'folder',
        parentId: null,
        order: 0,
        createdAt: 1,
        updatedAt: 1,
        github: { repository: 'owner/repo', branch: 'main', write: true },
    };
    const doc: TreeNode = {
        ...root,
        id: 'filter-doc',
        name: 'Draft',
        kind: 'doc',
        parentId: root.id,
        github: undefined,
        githubFile: { root: root.id, path: 'Draft.md', sha: 'known' },
    };
    await db.putNodes([root, doc], { [doc.id]: 'Unsynced draft' }, { allowGitHubInserts: true });
    await db.updateGitHubExclusions(root.id, () => '/Draft.md');
    const github = { ...root.github!, credentialId: 'named', authRevision: 'new' };
    await db.putNodes([{ ...root, github }], {}, { expectedGitHubRoot: root.id, replaceGitHubConnection: true });
    await db.putNodes([{ ...root, open: true }]);
    assert.equal((await db.getNode(root.id))?.githubExcludes, '/Draft.md');
    assert.deepEqual((await db.getNode(root.id))?.github, github);
    await assert.rejects(
        db.putNodes([doc], { [doc.id]: 'Outdated pull' }, { expectedGitHubRoot: root.id, expectedGitHubExcludes: '' }),
        /during this check/,
    );
    assert.equal(await db.getText(doc.id), 'Unsynced draft');
    await db.detachGitHubFolder(root.id);
    assert.equal((await db.getNode(root.id))?.githubExcludes, undefined);
    assert.equal(await db.getText(doc.id), 'Unsynced draft');
});

test('a pending GitHub deletion retains original bytes and prevents a stale pull from recreating the tree', async () => {
    const fields = { order: 0, createdAt: 1, updatedAt: 1 };
    const root: TreeNode = {
        ...fields,
        id: 'pending-root',
        name: 'Repository',
        kind: 'folder',
        parentId: null,
        github: { repository: 'owner/repo', branch: 'main', write: true },
    };
    const doc: TreeNode = {
        ...fields,
        id: 'pending-doc',
        name: 'Document',
        kind: 'doc',
        parentId: root.id,
        githubFile: { root: root.id, path: 'Document.md', sha: 'old' },
    };
    await db.putNodes([root, doc], { [doc.id]: 'Local content' }, { allowGitHubInserts: true });
    const job: StructuralJob = {
        id: 'job',
        node: doc.id,
        ids: [doc.id],
        kind: 'delete',
        name: doc.name,
        from: 'Document.md',
        folder: false,
        files: [{ id: doc.id, from: 'Document.md', sha: 'old' }],
    };
    await db.updateGitHubJobs(root.id, (jobs, nodes) => {
        const current = nodes.find((n) => n.id === doc.id)!;
        current.githubChange = { id: job.id, kind: job.kind };
        jobs.push(job);
        return { value: undefined, changed: [current] };
    });
    assert.equal(await db.getText(doc.id), 'Local content');
    assert.deepEqual(await db.getSide('github-jobs', root.id), [job]);
    assert.deepEqual((await db.getNode(root.id))?.githubReview, { pending: 1, blocked: 0 });
    await assert.rejects(
        db.putNodes([doc], { [doc.id]: 'Remote overwrite' }, { expectedGitHubRoot: root.id, expectedGitHubJobs: [] }),
        /queued during this check/,
    );
    await db.detachGitHubFolder(root.id);
    assert.equal((await db.getNode(doc.id))?.githubChange, undefined);
    assert.equal(await db.getText(doc.id), 'Local content');
    assert.equal(await db.getSide('github-jobs', root.id), undefined);
});

test('GitHub detach preserves the hierarchy and local content, and stale saves cannot restore its binding', async () => {
    const fields = { order: 0, createdAt: 1, updatedAt: 1 };
    const root: TreeNode = {
        ...fields,
        id: 'root',
        name: 'Repository',
        kind: 'folder',
        parentId: null,
        github: { repository: 'owner/repo', branch: 'main', write: true },
    };
    const nested: TreeNode = { ...fields, id: 'nested', name: 'Nested', kind: 'folder', parentId: root.id };
    const doc: TreeNode = {
        ...fields,
        id: 'doc',
        name: 'Document',
        kind: 'doc',
        parentId: nested.id,
        originId: 'original',
        pending: true,
        githubFile: { root: root.id, path: 'Nested/Document.md', sha: 'old', conflict: 'new' },
    };
    await db.putNodes(
        [root, nested, doc],
        { doc: 'Local unsynced edit' },
        {
            allowGitHubInserts: true,
            extras: {
                'github-auth:root': 'token',
                'github-index:root': 'index',
                'github-base:doc': 'Upstream',
                'github-conflict:doc': 'conflict',
                'base:doc': 'Earlier review',
                'vec:doc': ['index'],
            },
        },
    );
    await db.setMeta('github-poll:root:auth', { retryAt: 1 });
    await db.detachGitHubFolder(root.id);
    assert.equal((await db.getNode(root.id))?.github, undefined);
    assert.equal((await db.getNode(doc.id))?.githubFile, undefined);
    assert.deepEqual(await db.getNode(nested.id), nested);
    assert.equal((await db.getNode(doc.id))?.parentId, nested.id);
    assert.equal((await db.getNode(doc.id))?.originId, doc.originId);
    assert.equal(await db.getText(doc.id), 'Local unsynced edit');
    assert.equal(await db.getSide('base', doc.id), 'Earlier review');
    assert.deepEqual(await db.getSide('vec', doc.id), ['index']);
    for (const side of ['github-auth', 'github-index'] as const)
        assert.equal(await db.getSide(side, root.id), undefined);
    for (const side of ['github-base', 'github-conflict'] as const)
        assert.equal(await db.getSide(side, doc.id), undefined);
    assert.equal(await db.getMeta('github-poll:root:auth'), undefined);
    await assert.rejects(
        db.putNodes([root, doc], { doc: 'Stale pull' }, { expectedGitHubRoot: root.id }),
        /no longer synced/,
    );
    await db.putNodes([root, doc], { doc: 'Another tab saved a local edit' });
    assert.equal((await db.getNode(root.id))?.github, undefined);
    assert.equal((await db.getNode(doc.id))?.githubFile, undefined);
    assert.equal(await db.getText(doc.id), 'Another tab saved a local edit');
});
