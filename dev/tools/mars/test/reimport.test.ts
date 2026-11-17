import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import { indexedDB, IDBKeyRange } from 'fake-indexeddb';
import { incomingChoice, sourceBody } from '../src/share/reconcile.ts';
import { mergeDocuments, openThreads, joinComments } from '../src/share/merge.ts';
import { textBytes, type TreeNode } from '../src/tree/model.ts';
import { incomingFolder } from '../src/io/destination.ts';
import type { ImportItem } from '../src/io/reimport.ts';

Object.assign(globalThis, {
    indexedDB,
    IDBKeyRange,
    location: new URL('https://mars.test/'),
    BroadcastChannel: undefined,
});
Object.defineProperty(globalThis, 'navigator', { value: {}, configurable: true });
const db = await import('../src/store/db.ts');
const source = readFileSync(new URL('../src/io/reimport.ts', import.meta.url), 'utf8');
const code = transformSync(source.replace(/^import .*;$/gm, ''), { loader: 'ts', format: 'cjs' }).code;
const input = { check() {}, signal: new AbortController().signal };
const node = (name: string, path: string[]): TreeNode => ({
    id: crypto.randomUUID(),
    kind: 'doc',
    name,
    parentId: null,
    order: 0,
    createdAt: 1,
    updatedAt: 1,
    opened: true,
    importSource: { destinationId: null, path },
});
function setup() {
    const state = { nodes: [] as TreeNode[], currentId: null as string | null };
    let writes = 0;
    const context = {
        module: { exports: {} },
        JSON,
        Date,
        incomingChoice,
        sourceBody,
        mergeDocuments,
        openThreads,
        textBytes,
        incomingFolder,
        app: { state, node: (id: string) => state.nodes.find((n) => n.id === id), notify() {} },
        db: {
            ...db,
            putNodes: async (...args: Parameters<typeof db.putNodes>) => {
                writes++;
                return db.putNodes(...args);
            },
        },
        viewer: { flush: async () => {}, openDocument() {} },
        sync: {
            snapshot: async () => {
                throw new Error('Permission denied');
            },
        },
    };
    const api = runInNewContext(`${code}\nmodule.exports`, context) as typeof import('../src/io/reimport.ts');
    return { ...api, state, writes: () => writes };
}
const item = (sourcePath: string[], text: string): ImportItem => ({
    sourcePath,
    path: sourcePath.slice(0, -1),
    name: sourcePath.at(-1)!.replace(/\.\w+$/, ''),
    text,
});

test('import keys distinguish destination, complete nested paths, case, extensions, and Unicode segments', () => {
    const { importKey, legacyImportParent } = setup();
    const paths = [
        ['a', 'README.md'],
        ['b', 'README.md'],
        ['a', 'README.txt'],
        ['a', 'readme.md'],
        ['研究 & <proof>', 'README.md'],
    ];
    assert.equal(new Set(paths.map((path) => importKey({ destinationId: null, path }))).size, paths.length);
    assert.notEqual(
        importKey({ destinationId: 'other', path: paths[0] }),
        importKey({ destinationId: null, path: paths[0] }),
    );
    const folder = { ...node('a', []), kind: 'folder' as const, importSource: undefined };
    assert.equal(legacyImportParent([folder], null, ['a']), folder.id);
    assert.equal(legacyImportParent([folder], null, ['A']), undefined);
    assert.equal(legacyImportParent([folder, { ...folder, id: 'duplicate' }], null, ['a']), undefined);
});

test('a re-import from another destination finds the one earlier destination and its renamed nested folders', () => {
    const { earlierDestination, recordedFolders } = setup();
    const folder = (name: string, parentId: string | null): TreeNode => ({
        ...node(name, []),
        kind: 'folder',
        parentId,
        importSource: undefined,
    });
    const inbox = folder('Inbox', null),
        project = folder('Project', inbox.id),
        renamed = folder('My label', project.id),
        deep = folder('Deep', renamed.id),
        other = folder('Other', null);
    const doc = (path: string[], parentId: string, destinationId: string | null = inbox.id) => ({
        ...node(path.at(-1)!, path),
        parentId,
        importSource: { destinationId, path },
    });
    const nodes = [
        inbox,
        project,
        renamed,
        deep,
        other,
        doc(['Project', 'L1', 'a.md'], renamed.id),
        doc(['Project', 'L1', 'Deep', 'b.md'], deep.id),
        // Moved to another depth: it does not describe its source folders.
        doc(['Project', 'L1', 'Deep', 'moved.md'], project.id),
    ];
    const items = [item(['Project', 'L1', 'a.md'], ''), item(['Project', 'New.md'], '')];
    assert.deepEqual({ ...earlierDestination(nodes, items, other.id) }, { id: inbox.id, count: 1 });
    assert.equal(earlierDestination(nodes, items, inbox.id), undefined);
    assert.equal(earlierDestination(nodes, [item(['Project', 'New.md'], '')], other.id), undefined);
    assert.equal(
        earlierDestination([...nodes, doc(['Project', 'L1', 'a.md'], other.id, null)], items, other.id),
        undefined,
    );
    assert.deepEqual(
        Object.fromEntries(recordedFolders(nodes, inbox.id)),
        Object.fromEntries([
            ['/Project', project.id],
            ['/Project/L1', renamed.id],
            ['/Project/L1/Deep', deep.id],
        ]),
    );
    // Disagreeing documents leave the folder to the name lookup.
    const split = folder('L1', project.id);
    assert.equal(
        recordedFolders([...nodes, split, doc(['Project', 'L1', 'c.md'], split.id)], inbox.id).has('/Project/L1'),
        false,
    );
    assert.equal(recordedFolders(nodes, other.id).size, 0);
});

test('24-file re-import skips unchanged bytes and vectors, updates one identity, and retains the review baseline', async () => {
    const api = setup();
    const items = Array.from({ length: 24 }, (_, i) =>
        item([`Section ${i % 4}`, 'Nested', `Note ${i}.md`], `# Note ${i}\r\n\r\nOriginal.\r\n`),
    );
    for (const incoming of items) {
        const n = node(incoming.name, incoming.sourcePath);
        api.state.nodes.push(n);
        await db.putNodes(
            [n],
            { [n.id]: incoming.text },
            { extras: { [`opened:${n.id}`]: incoming.text, [`vec:${n.id}`]: { saved: true } } },
        );
    }
    const before = await Promise.all(api.state.nodes.map((n) => db.getDocument(n.id)));
    const changed = items.map((incoming, i) =>
        i === 7 ? { ...incoming, text: incoming.text.replace('Original.', 'Edited on disk.') } : incoming,
    );
    const plans = await api.planImport(changed, null, input);
    assert.equal(plans.filter((p) => p.question).length, 0);
    const results = [];
    for (const plan of plans) results.push(await api.updateImport(plan, input));
    assert.equal(results.filter((r) => r === 'updated').length, 1);
    assert.equal(results.filter((r) => r === 'unchanged').length, 23);
    assert.equal(api.writes(), 1);
    for (let i = 0; i < before.length; i++) {
        const id = before[i]!.node.id,
            after = (await db.getDocument(id))!;
        assert.deepEqual(await db.getSide('vec', id), { saved: true });
        if (i === 7) {
            assert.equal(after.text, changed[i].text);
            assert.equal(after.node.pending, true);
            assert.equal(await db.getSide('base', id), items[i].text);
            assert.equal(await db.getSide('opened', id), changed[i].text);
        } else assert.deepEqual(after, before[i]);
    }
});

test('body selection ignores comments and CRLF while retaining local-only edits and requiring both-side decisions', () => {
    assert.equal(incomingChoice('# A\r\n', '# A\n'), 'mine');
    assert.equal(incomingChoice('Local', 'Original', 'Original'), 'mine');
    assert.equal(incomingChoice('Original', 'Incoming', 'Original'), 'theirs');
    assert.equal(incomingChoice('Local', 'Incoming', 'Original'), 'conflict');
    assert.equal(incomingChoice('Local', 'Incoming'), 'conflict');
});

test('keep-my-text merges replies, advances the source, and an earlier review baseline survives later imports', async () => {
    const api = setup(),
        n = node('Review', ['Review.md']);
    const thread = { id: 'thread', quote: 'Original', comments: [{ id: 'one', body: 'Review' }] };
    const original = joinComments('Original', [thread]);
    const local = joinComments('My edit', [thread]);
    const incoming = joinComments('Disk edit', [
        { ...thread, comments: [...thread.comments, { id: 'two', body: 'Reply' }] },
    ]);
    api.state.nodes.push(n);
    await db.putNodes([n], { [n.id]: local }, { extras: { [`opened:${n.id}`]: original } });
    const [plan] = await api.planImport([item(['Review.md'], incoming)], null, input);
    assert.equal(plan.question, 'conflict');
    plan.choice = 'mine';
    await api.updateImport(plan, input);
    assert.match((await db.getText(n.id))!, /My edit/);
    assert.match((await db.getText(n.id))!, /Reply/);
    assert.equal(await db.getSide('opened', n.id), incoming);
    api.state.nodes = [(await db.getDocument(n.id))!.node];
    const next = await api.planImport([item(['Review.md'], incoming)], null, input);
    assert.equal(next[0].question, undefined);
    assert.equal(next[0].choice, 'mine');
    const baseline = 'Earlier review';
    const current = (await db.getDocument(n.id))!;
    await db.putNodes([{ ...current.node, pending: true }], {}, { extras: { [`base:${n.id}`]: baseline } });
    api.state.nodes = [(await db.getDocument(n.id))!.node];
    const [again] = await api.planImport([item(['Review.md'], 'Newer disk edit')], null, input);
    again.choice = 'theirs';
    await api.updateImport(again, input);
    assert.equal(await db.getSide('base', n.id), baseline);
});

test('legacy matching needs confirmation, changed labels retain recorded identity, and ambiguous paths never match', async () => {
    const api = setup(),
        n = node('Note', ['Note.md']);
    delete n.importSource;
    api.state.nodes.push(n);
    await db.putNodes([n], { [n.id]: 'Old' }, { extras: { [`opened:${n.id}`]: 'Old' } });
    const [legacy] = await api.planImport([item(['Note.md'], 'New')], null, input);
    assert.equal(legacy.question, 'legacy');
    assert.equal(legacy.choice, 'copy');
    legacy.choice = 'theirs';
    await api.updateImport(legacy, input);
    const stored = (await db.getDocument(n.id))!;
    await db.putNodes([{ ...stored.node, name: 'My label' }]);
    api.state.nodes = [(await db.getDocument(n.id))!.node];
    const [renamed] = await api.planImport([item(['Note.md'], 'New')], null, input);
    assert.equal(renamed.match!.node.id, n.id);
    assert.equal(renamed.question, undefined);
    const ambiguous = await api.planImport([item(['Note.md'], 'One'), item(['Note.md'], 'Two')], null, input);
    assert.ok(ambiguous.every((p) => p.ambiguous && !p.match && !p.source));
});

test('atomic incoming writes reject newer text, source, metadata, deleted rows, and uncloneable side data', async () => {
    for (const change of ['text', 'source', 'name', 'delete', 'clone']) {
        const n = node('Retained', ['Retained.md']);
        await db.putNodes([n], { [n.id]: 'Original' }, { extras: { [`opened:${n.id}`]: 'Original' } });
        const expected = { ...(await db.getDocument(n.id))!, opened: 'Original' };
        if (change === 'text') await db.putNodes([n], { [n.id]: 'Other tab' });
        if (change === 'source') await db.putSide('opened', n.id, 'Another source');
        if (change === 'name') await db.putNodes([{ ...n, name: 'Renamed' }]);
        if (change === 'delete') await db.deleteNodes([n.id]);
        const before = await db.getDocument(n.id),
            opened = await db.getSide('opened', n.id);
        await assert.rejects(
            db.putNodes(
                [{ ...n, pending: true }],
                { [n.id]: 'Incoming' },
                {
                    expected: [expected],
                    extras: {
                        [`opened:${n.id}`]: 'Incoming',
                        [`base:${n.id}`]: change === 'clone' ? () => {} : 'Original',
                    },
                },
            ),
        );
        assert.deepEqual(await db.getDocument(n.id), before);
        assert.equal(await db.getSide('opened', n.id), opened);
        assert.equal(await db.getSide('base', n.id), undefined);
    }
});

test('a refused Sync import retains document, source, binding, and review side data', async () => {
    const api = setup(),
        n = node('Connected', ['Connected.md']);
    n.connection = {
        collection: 'collection',
        member: 'member',
        document: 'document',
        epoch: 1,
        kind: 'sync',
        permission: 'owner',
    };
    api.state.nodes.push(n);
    await db.putNodes([n], { [n.id]: 'Original' }, { extras: { [`opened:${n.id}`]: 'Original' } });
    const before = await db.getDocument(n.id),
        [plan] = await api.planImport([item(['Connected.md'], 'Incoming')], null, input);
    await assert.rejects(api.updateImport(plan, input), /Permission denied/);
    assert.deepEqual(await db.getDocument(n.id), before);
    assert.equal(await db.getSide('opened', n.id), 'Original');
    assert.equal(await db.getSide('base', n.id), undefined);
});
