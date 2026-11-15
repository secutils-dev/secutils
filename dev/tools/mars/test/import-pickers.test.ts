import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import { incomingFolder } from '../src/io/destination.ts';
import type { TreeNode } from '../src/tree/model.ts';

const source = readFileSync(new URL('../src/io/pickers.ts', import.meta.url), 'utf8');

function setup() {
    const inputs = new Map(
        ['importInput', 'importFolderInput'].map((id) => [
            id,
            {
                files: [] as object[],
                value: '',
                events: new Map<string, () => void>(),
                clicked: false,
                click() {
                    this.clicked = true;
                },
                addEventListener(type: string, listener: () => void) {
                    this.events.set(type, listener);
                },
            },
        ]),
    );
    const state = { folderId: 'selected' };
    const imports: { files: object[]; parentId: string | null }[] = [];
    // The module only needs two inputs. Stub its imports and evaluate the actual picker handlers.
    const code = transformSync(source.replace(/^import .*;$/gm, ''), { loader: 'ts' }).code.replace(/^export /gm, '');
    const pickers = runInNewContext(`${code}\n({pickImport, installImportPickers})`, {
        app: { state },
        $: (id: string) => inputs.get(id),
        importFiles: (files: object[], parentId: string | null) => imports.push({ files, parentId }),
    });
    pickers.installImportPickers();
    const choose = (id: string) => {
        const input = inputs.get(id)!;
        input.files = [{ name: 'LETTERS.md' }];
        input.value = 'LETTERS.md';
        input.events.get('change')!();
        assert.equal(input.value, '');
        return imports.at(-1)!;
    };
    return { inputs, state, imports, pickers, choose };
}

test('file and folder pickers retain separate targets across changes to the selected folder', () => {
    const { inputs, state, pickers, choose } = setup();
    pickers.pickImport('files', 'deep-folder');
    pickers.pickImport('folder', 'other-folder');
    assert.equal(inputs.get('importInput')!.clicked, true);
    state.folderId = 'changed';
    assert.equal(choose('importInput').parentId, 'deep-folder');
    assert.equal(choose('importFolderInput').parentId, 'other-folder');
    assert.equal(state.folderId, 'changed');
});

test('an explicit root target remains null, and cancellation or selection clears the pending target', () => {
    const { inputs, state, pickers, choose, imports } = setup();
    pickers.pickImport('files', null);
    assert.equal(choose('importInput').parentId, null);
    state.folderId = 'selected-again';
    assert.equal(choose('importInput').parentId, 'selected-again');
    pickers.pickImport('folder', 'cancelled-target');
    inputs.get('importFolderInput')!.events.get('cancel')!();
    assert.equal(choose('importFolderInput').parentId, 'selected-again');
    const input = inputs.get('importInput')!;
    input.files = [];
    input.events.get('change')!();
    assert.equal(imports.length, 3);
});

test('shared import actions target the requested folder and reject read-only or ended ancestors', () => {
    const sidebar = readFileSync(new URL('../src/sidebar/sidebar.ts', import.meta.url), 'utf8');
    const start = sidebar.indexOf('function importMenu(');
    const code = transformSync(sidebar.slice(start, sidebar.indexOf('\n}\n', start) + 2), { loader: 'ts' }).code;
    const root: TreeNode = {
        id: 'root',
        parentId: null,
        kind: 'folder',
        name: 'Root',
        order: 0,
        createdAt: 1,
        updatedAt: 1,
        connection: {
            collection: 'collection',
            member: 'member',
            document: 'root',
            kind: 'sync',
            permission: 'owner',
            epoch: 1,
        },
    };
    const child: TreeNode = { ...root, id: 'child', parentId: 'root', connection: undefined };
    const picked: { kind: string; parentId: string | null }[] = [];
    const menuFor = runInNewContext(`${code}\nimportMenu`, {
        app: { state: { nodes: [root, child], folderId: 'unrelated' } },
        incomingFolder,
        pickImport: (kind: string, parentId: string | null) => picked.push({ kind, parentId }),
    });
    for (const parentId of [null, child.id]) {
        const entries = menuFor(parentId);
        assert.equal(entries[0].label, 'Import files…');
        assert.equal('sub' in entries[0], false);
        assert.equal(entries[1].label, 'Import a folder…');
        assert.equal(entries[0].disabled, false);
        entries[0].run();
        entries[1].run();
        assert.deepEqual(picked.slice(-2), [
            { kind: 'files', parentId },
            { kind: 'folder', parentId },
        ]);
    }
    for (const permission of ['view', 'comment'] as const) {
        root.connection!.permission = permission;
        assert.equal(
            menuFor(child.id).every((entry: { disabled: boolean }) => entry.disabled),
            true,
        );
    }
    root.connection!.permission = 'edit';
    root.connection!.phase = 'ended';
    assert.equal(menuFor(child.id)[0].disabled, true);
    assert.equal(menuFor('deleted-folder')[0].disabled, true);
});
