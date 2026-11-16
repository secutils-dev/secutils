import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import * as models from '../src/search/models.ts';
import * as fingerprint from '../src/search/fingerprint.ts';

const source = readFileSync(new URL('../src/search/embed.ts', import.meta.url), 'utf8');
const code = transformSync(source, { loader: 'ts', format: 'cjs' }).code;
const text =
    '# Large document\n\n' +
    Array.from(
        { length: 40 },
        (_, i) =>
            `## Section ${i}\n\nThe cache for section ${i} expires after fifteen minutes. Review authorization before deployment.`,
    ).join('\n\n');
type Entry = { input: string; chunks: { v: Float32Array }[] };
type Api = typeof import('../src/search/embed.ts');

// Run the real lifecycle and index loop with deterministic worker responses and storage snapshots.
function harness(nativeYield = true) {
    const settings = new Map([['mars.smartSearch', 'fast']]);
    const entries = new Map<string, Entry>();
    const nodes = [{ id: 'large', kind: 'doc', name: 'Large', updatedAt: 1 }];
    const texts = new Map([['large', text]]);
    const listeners = new Map<string, () => void>();
    const batches: string[][] = [];
    let storage = 0,
        locks = 0,
        yields = 0,
        loads = 0;
    let onBatch = () => {},
        onYield = () => {};
    const document = { hidden: false, addEventListener: (name: string, fn: () => void) => listeners.set(name, fn) };
    const localStorage = {
        getItem: (key: string) => settings.get(key) ?? null,
        setItem: (key: string, value: string) => settings.set(key, value),
        removeItem: (key: string) => settings.delete(key),
    };
    const yieldTask = async () => {
        yields++;
        onYield();
    };
    const module = { exports: {} as Api };
    runInNewContext(code, {
        module,
        exports: module.exports,
        require: (path: string) => {
            if (path === './models.ts') return models;
            if (path === './fingerprint.ts') return fingerprint;
            if (path === '../app.ts')
                return { state: { nodes }, node: (id: string) => nodes.find((n) => n.id === id), onChange: () => {} };
            if (path === '../store/db.ts')
                return {
                    storageGeneration: () => storage,
                    isTemporaryStorage: () => false,
                    allSides: async () => [...entries.values()],
                    getSide: async (_kind: string, id: string) => entries.get(id),
                    putSide: async (_kind: string, id: string, entry: Entry) => entries.set(id, entry),
                    clearSides: async () => entries.clear(),
                    onSideChange: () => {},
                    getDocument: async (id: string) => {
                        const node = nodes.find((n) => n.id === id);
                        return node ? { node: { ...node }, text: texts.get(id) } : null;
                    },
                };
            throw new Error(`Unexpected import: ${path}`);
        },
        window: {
            addEventListener: (name: string, fn: () => void) => listeners.set(name, fn),
            ...(nativeYield ? { scheduler: { yield: yieldTask } } : {}),
        },
        document,
        localStorage,
        navigator: {
            gpu: { requestAdapter: async () => ({ features: new Set(['shader-f16']) }) },
            locks: {
                request: async (_name: string, run: () => Promise<void>) => {
                    locks++;
                    try {
                        await run();
                    } finally {
                        locks--;
                    }
                },
            },
        },
        Blob,
        URL: { createObjectURL: () => 'blob:test' },
        Float32Array,
        setTimeout: (fn: () => void) => {
            yields++;
            onYield();
            queueMicrotask(fn);
        },
        clearTimeout: () => {},
        Worker: class {
            onmessage: ((event: { data: unknown }) => void) | null = null;
            postMessage(msg: { id: number; type: string; texts?: string[] }) {
                if (msg.type === 'load') loads++;
                else {
                    batches.push(msg.texts!);
                    onBatch();
                }
                // Vectors depend on each input, never its batch or execution order.
                const data = new Float32Array((msg.texts?.length ?? 0) * 384);
                for (const [i, input] of (msg.texts ?? []).entries()) {
                    const seed = [...input].reduce((sum, char) => sum + char.charCodeAt(0), 0);
                    for (let j = 0; j < 384; j++) data[i * 384 + j] = ((seed + j) % 19) + 1;
                }
                queueMicrotask(() => this.onmessage?.({ data: { id: msg.id, dims: 384, data, device: 'wasm' } }));
            }
            terminate() {}
        },
    });
    return {
        api: module.exports,
        entries,
        nodes,
        texts,
        settings,
        batches,
        onBatch: (fn: () => void) => {
            onBatch = fn;
        },
        onYield: (fn: () => void) => {
            onYield = fn;
        },
        hide: (hidden: boolean) => {
            document.hidden = hidden;
            listeners.get('visibilitychange')!();
        },
        switchStorage: () => {
            storage++;
            entries.clear();
        },
        locks: () => locks,
        yields: () => yields,
        loads: () => loads,
    };
}

const inputs = (h: ReturnType<typeof harness>) => h.batches.flat();
const vectors = (entry: Entry) => JSON.stringify(entry.chunks.map((c) => [...c.v]));

for (const nativeYield of [true, false]) {
    test(`Pause at a batch boundary resumes without redundant inference (${nativeYield ? 'scheduler' : 'timer'} yield)`, async () => {
        const baseline = harness(nativeYield);
        await baseline.api.reindex();
        const h = harness(nativeYield);
        h.onYield(() => {
            if (h.yields() === 1) h.api.pause();
        });
        await h.api.reindex();
        assert.equal(h.batches.length, 1);
        assert.equal(h.batches[0].length, 8);
        assert.equal(h.locks(), 0);
        assert.equal(h.entries.size, 0, 'an incomplete entry never becomes searchable');
        await h.api.resume();
        assert.deepEqual(inputs(h), inputs(baseline));
        assert.equal(vectors(h.entries.get('large')!), vectors(baseline.entries.get('large')!));
        assert.equal(h.yields(), h.batches.length);
    });
}

test('hidden tabs release the lock, resume retained batches, and keep a manual pause', async () => {
    const h = harness();
    h.onBatch(() => {
        if (h.batches.length === 1) h.hide(true);
    });
    await h.api.reindex();
    assert.equal(h.batches.length, 1);
    assert.equal(h.locks(), 0);
    assert.equal(h.api.isPaused(), false);
    await h.api.reindex();
    assert.equal(h.batches.length, 1);
    h.api.pause();
    h.hide(false);
    await h.api.reindex();
    assert.equal(h.batches.length, 1);
    assert.equal(h.api.isPaused(), true);
    await h.api.resume();
    const baseline = harness();
    await baseline.api.reindex();
    assert.deepEqual(inputs(h), inputs(baseline));
    assert.equal(vectors(h.entries.get('large')!), vectors(baseline.entries.get('large')!));
});

test('visibility alone resumes indexing and a hidden startup does not load a model', async () => {
    const h = harness();
    h.hide(true);
    await h.api.reindex();
    assert.equal(h.loads(), 0);
    h.hide(false);
    await h.api.reindex();
    assert.equal(h.entries.size, 1);
    assert.equal(h.settings.has('mars.indexingPaused'), false);
});

test('a remote pause stops batches even before its storage event arrives', async () => {
    const h = harness();
    h.onBatch(() => {
        if (h.batches.length === 1) h.settings.set('mars.indexingPaused', '1');
    });
    await h.api.reindex();
    assert.equal(h.batches.length, 1);
    assert.equal(h.locks(), 0);
    await h.api.resume();
    const baseline = harness();
    await baseline.api.reindex();
    assert.deepEqual(inputs(h), inputs(baseline));
});

test('an existing saved index remains valid without loading a model or rebuilding', async () => {
    const baseline = harness();
    await baseline.api.reindex();
    const h = harness();
    const saved = baseline.entries.get('large')!;
    h.entries.set('large', saved);
    await h.api.reindex();
    assert.equal(h.loads(), 0);
    assert.equal(h.batches.length, 0);
    assert.equal(h.entries.get('large'), saved);
});

test('other stale documents do not evict retained work before it resumes', async () => {
    const h = harness();
    h.onBatch(() => {
        if (h.batches.length === 1) h.api.pause();
    });
    await h.api.reindex();
    h.nodes.unshift({ id: 'earlier', kind: 'doc', name: 'Earlier', updatedAt: 1 });
    h.texts.set('earlier', 'An earlier document also needs a new search vector.');
    await h.api.resume();
    const baseline = harness();
    await baseline.api.reindex();
    assert.deepEqual(inputs(h).slice(0, inputs(baseline).length), inputs(baseline));
    assert.equal(h.entries.size, 2);
});

for (const change of ['text', 'name', 'delete', 'storage', 'rebuild', 'model'] as const) {
    test(`${change} during suspension cannot publish stale partial vectors`, async () => {
        const h = harness();
        h.onBatch(() => {
            if (h.batches.length === 1) h.api.pause();
        });
        await h.api.reindex();
        const firstInputs = [...h.batches[0]];
        if (change === 'text') {
            h.texts.set('large', text.replaceAll('cache', 'session'));
            h.nodes[0].updatedAt++;
        }
        if (change === 'name') {
            h.nodes[0].name = 'Renamed';
            h.nodes[0].updatedAt++;
        }
        if (change === 'delete') h.nodes.splice(0);
        if (change === 'storage') h.switchStorage();
        if (change === 'rebuild') await h.api.enable('fast', true);
        if (change === 'model') await h.api.enable('best');
        await h.api.resume();
        if (change === 'delete') {
            assert.equal(h.entries.size, 0);
            assert.equal(h.batches.length, 1);
            return;
        }
        const entry = h.entries.get('large')!;
        assert.ok(entry);
        assert.equal(
            entry.input,
            await fingerprint.inputFingerprint(
                models.MODELS[change === 'model' ? 'best' : 'fast'],
                h.nodes[0].name,
                models.indexChunks(h.texts.get('large')!),
            ),
        );
        if (change === 'name')
            assert.notDeepEqual(h.batches[1], firstInputs, 'GTE rename reuses completed batch inputs');
        if (change === 'rebuild' || change === 'storage')
            assert.deepEqual(h.batches[1], firstInputs, 'invalid generation requires fresh inference');
    });
}

test('an edit during inference stops obsolete batches and queues the current snapshot', async () => {
    const h = harness();
    h.onBatch(() => {
        if (h.batches.length !== 1) return;
        h.texts.set('large', 'The replacement document has entirely different model inputs.');
        h.nodes[0].updatedAt++;
    });
    await h.api.reindex();
    await h.api.reindex();
    assert.equal(h.batches.length, 2);
    assert.equal(h.entries.get('large')!.chunks.length, 1);
});
