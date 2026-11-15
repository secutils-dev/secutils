import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import { ancestors, descendants, type TreeNode } from '../src/tree/model.ts';
import { incomingFolder } from '../src/io/destination.ts';
import { decodeState, encodeState, openSealed, parseSharedPath, sealBundle } from '../src/share/codec.ts';
import { validWidth } from '../src/export/settings.ts';
import { checkInput, InputController } from '../src/io/input.ts';

const source = readFileSync(new URL('../src/share/share.ts', import.meta.url), 'utf8');
const node = (id: string, name: string, parentId: string | null, kind: 'doc' | 'folder' = 'folder'): TreeNode => ({
    id,
    name,
    parentId,
    kind,
    order: 0,
    createdAt: 1,
    updatedAt: 1,
});

test('single-document snapshot bundles preserve exact saved names and folder paths without sibling text', async () => {
    const nodes = [
        node('root', 'Local', null),
        node('nested', '研究 & "evidence"', 'root'),
        node('doc', 'Lease evidence.md', 'nested', 'doc'),
        node('sibling', 'Private sibling', 'nested', 'doc'),
    ];
    const reads: string[] = [];
    const code = source.slice(source.indexOf('async function bundleFor'), source.indexOf('// ---- Short links'));
    const api = runInNewContext(`${transformSync(code, { loader: 'ts' }).code}\n({bundleFor, fullLink})`, {
        app: {
            node: (id: string) => nodes.find((node) => node.id === id),
            state: { nodes, currentId: 'doc' },
            docText: async (id: string) => {
                reads.push(id);
                return '---\ntitle: Misleading title\n---\n# A different heading\n';
            },
        },
        db: { isTemporaryStorage: () => false },
        viewer: { exportReadingWidth: () => 980, defaultReadingWidth: () => 800 },
        ancestors,
        descendants,
        validWidth,
        encodeState,
        URLSearchParams,
        base: () => 'https://mars.example/',
    });
    const bundle = JSON.parse(JSON.stringify(await api.bundleFor('doc')));
    assert.equal(bundle.kind, 'doc');
    assert.equal(bundle.name, 'Lease evidence.md');
    assert.equal(bundle.docs.length, 1);
    assert.equal(bundle.docs[0].name, 'Lease evidence.md');
    assert.deepEqual(bundle.docs[0].path, ['Local', '研究 & "evidence"']);
    assert.deepEqual(reads, ['doc']);
    const sealed = await sealBundle(bundle);
    assert.deepEqual(await openSealed(sealed.secret, sealed.blob), bundle);
    const params = new URLSearchParams(new URL(await api.fullLink(bundle)).hash.slice(1));
    assert.equal(params.get('name'), 'Lease evidence.md');
    assert.deepEqual(parseSharedPath((await decodeState(params.get('p')!))!), bundle.docs[0].path);
    assert.equal(params.get('w'), '980');
    assert.equal(await decodeState(params.get('md')!), bundle.docs[0].text);
    nodes[1].name = 'Renamed research';
    const renamed = await api.bundleFor('doc');
    assert.equal(renamed.docs[0].path[1], 'Renamed research');
    assert.notEqual(JSON.stringify(renamed), JSON.stringify(bundle));
    nodes[2].parentId = 'root';
    const moved = await api.bundleFor('doc');
    assert.equal(JSON.stringify(moved.docs[0].path), '["Local"]');
    assert.deepEqual(bundle.docs[0].path, ['Local', '研究 & "evidence"']);
});

test('snapshot folder paths round-trip empty, Unicode, and punctuation names and reject malformed inputs', async () => {
    for (const path of [[], ['Local', 'Deep'], ['研究', 'A/B', '..', '<img onerror=alert(1)>']])
        assert.deepEqual(parseSharedPath((await decodeState(await encodeState(JSON.stringify(path))))!), path);
    for (const json of ['null', '{}', '"Local"', '[1]', '["Local",null]', '[{"name":"Local"}]', 'bad json'])
        assert.equal(parseSharedPath(json), null);
});

test('incoming document snapshots recreate only their path and leave known copies in place', async () => {
    const nodes = [node('incoming', 'Inbox', null)];
    const created: { name: string; parentId: string }[] = [];
    let existing: TreeNode | null = null;
    let updates = 0;
    const start = source.indexOf('async function receiveDoc(');
    const code = source.slice(start, source.indexOf('\n}', start) + 2);
    const receive = runInNewContext(`${transformSync(code, { loader: 'ts' }).code}\nreceiveDoc`, {
        app: {
            state: { nodes },
            node: (id: string) => nodes.find((node) => node.id === id),
            defaultIncomingFolder: () => nodes[0],
            findOrCreateFolder: async (name: string, parentId: string) => {
                const folder = node(`folder-${nodes.length}`, name, parentId);
                nodes.push(folder);
                return folder;
            },
            uniqueName: (name: string) => name,
            createDoc: async (doc: { name: string; parentId: string }) => created.push(doc),
        },
        findDoc: () => existing,
        db: { isTemporaryStorage: () => false },
        findLikely: async () => null,
        updateExisting: async () => updates++,
        checkInput() {},
        incomingFolder,
        viewer: { toast() {} },
    });
    const doc = {
        path: ['Local', 'Research'],
        name: 'LETTERS',
        originId: 'origin',
        text: '# Emails',
        readingWidth: 980,
    };
    await receive(doc);
    assert.deepEqual(
        nodes.map(({ name, parentId }) => ({ name, parentId })),
        [
            { name: 'Inbox', parentId: null },
            { name: 'Local', parentId: 'incoming' },
            { name: 'Research', parentId: 'folder-1' },
        ],
    );
    assert.equal(created[0].name, 'LETTERS');
    assert.equal(created[0].parentId, 'folder-2');
    existing = node('existing', 'My local label', 'incoming', 'doc');
    await receive({ ...doc, name: 'Changed sender name', path: ['A different path'] });
    assert.equal(updates, 1);
    assert.equal(created.length, 1);
    assert.equal(nodes.length, 3);
    assert.equal(existing.name, 'My local label');
    assert.equal(existing.parentId, 'incoming');
});

test('snapshot paths cannot create documents in read-only folders or after an input was superseded', async () => {
    for (const mode of ['read-only', 'superseded']) {
        const root = node('incoming', 'Inbox', null);
        const folder = node('target', 'Local', root.id);
        if (mode === 'read-only')
            folder.connection = {
                collection: 'collection',
                member: 'member',
                document: folder.id,
                kind: 'sync',
                permission: 'view',
                epoch: 1,
            };
        const inputs = new InputController();
        const input = inputs.begin();
        let folders = 0;
        let documents = 0;
        const start = source.indexOf('async function receiveDoc(');
        const code = source.slice(start, source.indexOf('\n}', start) + 2);
        const receive = runInNewContext(`${transformSync(code, { loader: 'ts' }).code}\nreceiveDoc`, {
            app: {
                state: { nodes: [root, folder] },
                defaultIncomingFolder: () => root,
                findOrCreateFolder: async () => {
                    folders++;
                    if (mode === 'superseded') inputs.begin();
                    return folder;
                },
                createDoc: async () => documents++,
            },
            findDoc: () => null,
            db: { isTemporaryStorage: () => false },
            findLikely: async () => null,
            checkInput,
            incomingFolder,
        });
        await assert.rejects(
            receive({ path: ['Local', 'Deep'], name: 'LETTERS', originId: 'origin', text: '# Emails' }, input),
            mode === 'superseded' ? { name: 'AbortError' } : /cannot receive documents/,
        );
        assert.equal(folders, 1);
        assert.equal(documents, 0);
    }
});
