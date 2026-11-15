import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import { ancestors, type TreeNode } from '../src/tree/model.ts';

const source = readFileSync(new URL('../src/app.ts', import.meta.url), 'utf8');
const start = source.indexOf('export function rename(');
const code = source
    .slice(start, source.lastIndexOf('/**', source.indexOf('export async function move(', start)))
    .replace('export function', 'function');

// The app imports DOM singletons. Evaluate its rename implementation with the same serialized
// queue and observable storage/viewer boundaries, rather than replacing its behavior in the test.
function fixture(kind: 'doc' | 'folder' = 'doc', temporary = false) {
    const item: TreeNode = {
        id: 'item',
        name: 'Original',
        parentId: null,
        kind,
        order: 0,
        createdAt: 1,
        updatedAt: 1,
        originId: 'origin',
        pending: true,
        threads: 2,
    };
    const state = { nodes: [item], currentId: 'item' as string | null };
    const writes: TreeNode[] = [];
    const published: string[] = [];
    const titles: string[] = [];
    const routes: (string | null)[] = [];
    let generation = 0;
    let fail = false;
    let queue: Promise<unknown> = Promise.resolve();
    const api = runInNewContext(`${transformSync(code, { loader: 'ts' }).code}\nrename`, {
        state,
        node: (id: string) => state.nodes.find((n) => n.id === id),
        now: () => 100,
        emit() {},
        ancestors,
        serializePersistence: (run: () => Promise<unknown>) => {
            const task = queue.then(run);
            queue = task.catch(() => {});
            return task;
        },
        save: async (nodes: TreeNode[]) => {
            if (fail) throw new Error('Storage failed');
            writes.push(...nodes.map((n) => ({ ...n })));
        },
        sync: {
            metadata: async (id: string) => {
                if (fail) throw new Error('Metadata failed');
                published.push(state.nodes.find((n) => n.id === id)!.name);
            },
        },
        viewer: { renameDocument: (name: string) => titles.push(name) },
        db: { isTemporaryStorage: () => temporary, storageGeneration: () => generation },
        setRoute: async (id: string | null) => routes.push(id),
    }) as (id: string, name: string) => Promise<(() => Promise<boolean>) | null>;
    return {
        item,
        state,
        writes,
        published,
        titles,
        routes,
        rename: (name: string) => api(item.id, name),
        switchStorage: () => generation++,
        fail: (value: boolean) => (fail = value),
    };
}

for (const kind of ['folder', 'doc'] as const)
    test(`${kind} rename undo restores only the name, once, and preserves intervening edits`, async () => {
        const f = fixture(kind);
        const undo = (await f.rename('  New name  '))!;
        assert.equal(f.item.name, 'New name');
        f.item.threads = 3;
        assert.equal(await undo(), true);
        assert.equal(f.item.name, 'Original');
        assert.equal(f.item.threads, 3);
        assert.equal(f.item.pending, true);
        assert.equal(f.item.originId, 'origin');
        assert.deepEqual(
            f.writes.map((n) => n.name),
            ['New name', 'Original'],
        );
        assert.deepEqual(f.titles, ['New name', 'Original']);
        assert.equal(await undo(), false);
    });

test('blank and trimmed unchanged names have no write or undo', async () => {
    const f = fixture();
    for (const name of ['', '  ', 'Original', ' Original ']) assert.equal(await f.rename(name), null);
    assert.equal(f.writes.length, 0);
});

test('rename undo reads the current node and refuses a later rename, deletion, or storage switch', async () => {
    for (const change of ['rename', 'delete', 'storage']) {
        const f = fixture();
        const undo = (await f.rename('New name'))!;
        if (change === 'rename') f.state.nodes = [{ ...f.item, name: 'Another tab' }];
        else if (change === 'delete') f.state.nodes = [];
        else f.switchStorage();
        assert.equal(await undo(), false);
        assert.equal(f.writes.length, 1);
    }
    const f = fixture();
    const undo = (await f.rename('New name'))!;
    f.state.nodes = [{ ...f.item, threads: 5 }];
    assert.equal(await undo(), true);
    assert.equal(f.state.nodes[0].name, 'Original');
    assert.equal(f.state.nodes[0].threads, 5);
});

test('connected rename and undo publish metadata and respect current access', async () => {
    const f = fixture();
    f.item.connection = {
        collection: 'collection',
        member: 'member',
        document: 'remote',
        kind: 'sync',
        permission: 'owner',
        epoch: 1,
    };
    const undo = (await f.rename('New name'))!;
    f.item.connection.permission = 'view';
    assert.equal(await undo(), false);
    assert.equal(await f.rename('Forbidden'), null);
    f.item.connection.permission = 'edit';
    f.item.connection.phase = 'ended';
    assert.equal(await undo(), false);
    delete f.item.connection.phase;
    assert.equal(await undo(), true);
    assert.deepEqual(f.published, ['New name', 'Original']);
    assert.equal(f.writes.length, 0);
});

test('a failed rename retains the old name and a failed undo can be retried', async () => {
    const f = fixture();
    f.fail(true);
    await assert.rejects(f.rename('New name'), /Storage failed/);
    assert.equal(f.item.name, 'Original');
    assert.equal(f.item.updatedAt, 1);
    f.fail(false);
    const undo = (await f.rename('New name'))!;
    f.fail(true);
    await assert.rejects(undo(), /Storage failed/);
    assert.equal(f.item.name, 'New name');
    f.fail(false);
    assert.equal(await undo(), true);
});

test('temporary document and ancestor-folder renames update the recovery route on undo', async () => {
    for (const kind of ['doc', 'folder'] as const) {
        const f = fixture(kind, true);
        if (kind === 'folder') {
            f.state.nodes.push({ ...f.item, id: 'child', kind: 'doc', parentId: 'item' });
            f.state.currentId = 'child';
        }
        const undo = (await f.rename('New name'))!;
        assert.equal(await undo(), true);
        assert.deepEqual(f.routes, [f.state.currentId, f.state.currentId]);
    }
});
