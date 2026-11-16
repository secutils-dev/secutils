// Smart (semantic) search: an embedding model runs in a module worker via Transformers.js,
// lazy-loaded from jsDelivr, with weights from Hugging Face cached by the browser. The index
// (one `vec:<id>` entry per document) lives in IndexedDB and follows document edits.
//
// This module owns the worker, the model lifecycle, the stored index, and the on, off, and pause
// settings. The model table, the input format, and ranking live in `models.ts`.
// - Modification timestamps schedule a check; the hash of exact model inputs decides whether
//   vectors need replacement. Snippets and source positions can update without inference.
// - One pass runs per tab. A Web Lock and a shared-cache recheck prevent duplicate inference
//   across tabs. A change during a pass queues exactly one more.
// - Pause and hidden tabs stop at batch boundaries and release the document lock. One unfinished
//   document keeps its vectors in memory; only complete, current documents enter the saved index.
// - Network: the Transformers.js module comes from jsDelivr at a pinned version, and the model
//   files come from Hugging Face. Document text never leaves the browser.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import {
    createEncoder,
    type Gpu,
    type Hit,
    indexChunks,
    MODELS,
    type ModelKey,
    onnxFile,
    passageInput,
    placement,
    queryInput,
    rank,
    toVectors,
    TRANSFORMERS_VERSION,
} from './models.ts';
import { inputFingerprint } from './fingerprint.ts';

export { MODELS, type Hit, type ModelKey };

/** The Transformers.js module that the worker imports. The version is pinned in `models.ts`. */
const TRANSFORMERS_URL = `https://cdn.jsdelivr.net/npm/@huggingface/transformers@${TRANSFORMERS_VERSION}`;

// Bump when what gets embedded, or how, changes, so stored indexes rebuild. A new model or
// quantization for a key rebuilds too.
const INDEX_VERSION = 5;
/**
 * The version tag includes the chunking version, model ID, and both supported quantizations.
 */
const indexVersion = (key: ModelKey) =>
    `${INDEX_VERSION}:${MODELS[key].id}:${MODELS[key].dtype}:${MODELS[key].gpuDtype ?? ''}`;
/** `localStorage` key: the selected `ModelKey`. Smart search is off when the key is absent or unknown. */
const SETTING_KEY = 'mars.smartSearch';
/** `localStorage` key: '1' while indexing is paused. It applies to every MARS tab of this browser. */
const PAUSE_KEY = 'mars.indexingPaused';
const readPaused = () => {
    try {
        return localStorage.getItem(PAUSE_KEY) === '1';
    } catch {
        return false;
    }
};
let paused = readPaused();
/** The selected model, or null when smart search is off or storage is unavailable. */
export function enabledModel(): ModelKey | null {
    try {
        const v = localStorage.getItem(SETTING_KEY);
        return v && v in MODELS ? (v as ModelKey) : null;
    } catch {
        return null;
    }
}

/**
 * The stored index of one document (`vec:<id>`). `v` is the `indexVersion` of the write, and
 * `at` is the `updatedAt` of the document when its text was read. `chunks[].text` is the
 * passage text cut to 220 characters, for the result list.
 */
interface Entry {
    id: string;
    model: ModelKey;
    v?: number | string;
    at: number;
    input?: string;
    chunks: { start: number; text: string; v: Float32Array }[];
}

// The entries of the selected model in memory, by document id. `loadIndex` fills it from storage.
const index = new Map<string, Entry>();
// What smart search is doing: `label` fits the sidebar badge, `text` is the full sentence.
export type Status = {
    state: 'download' | 'prepare' | 'index' | 'paused' | 'error';
    label: string;
    text: string;
} | null;
const statusListeners = new Set<(s: Status) => void>();
/** Registers a status listener and calls it at once with the current status. `null` means idle. */
export const onStatus = (fn: (s: Status) => void) => {
    statusListeners.add(fn);
    fn(currentStatus);
};
let currentStatus: Status = null;
const status = (s: Status) => {
    currentStatus = s;
    for (const fn of statusListeners) fn(s);
};
const PAUSED: Status = { state: 'paused', label: 'Paused', text: 'Smart-search indexing is paused' };
const HIDDEN: Status = {
    state: 'paused',
    label: 'Paused',
    text: 'Smart-search indexing resumes when this tab is visible',
};
// Visibility suspends work without changing the reader's saved Pause setting.
const suspension = () => (paused || readPaused() ? PAUSED : document.hidden ? HIDDEN : null);
if (paused && enabledModel()) currentStatus = PAUSED;
const unavailable = (err: unknown): Status => ({
    state: 'error',
    label: 'Unavailable',
    text: `Smart search is unavailable: ${(err as Error)?.message ?? err}`,
});

// ---- Worker ----
// Source of the module worker. Messages: `{ id, type: 'load', ... }` loads a model and answers
// `{ id, ok, device }`, with `{ id, progress }` messages before it. `{ id, type: 'embed', texts }`
// answers `{ id, ok, device, dims, data }`. A failure answers `{ id, error }`.
const WORKER = `
import * as tf from '${TRANSFORMERS_URL}';
tf.env.allowLocalModels = false;
let embed;
const createEncoder = ${createEncoder.toString()};
const create = (data, device, progress) => createEncoder(tf, data, device, progress);
// The main thread picks WebGPU only after gpuInfo(). A WebGPU failure, at load or first use,
// falls back to WebAssembly with the CPU weights (\`cpuDtype\`), unless the model is GPU-only.
let loadData, device;
const fallback = (err) => { if (device === 'wasm' || loadData.gpuOnly) throw err; device = 'wasm'; loadData = { ...loadData, dtype: loadData.cpuDtype }; };
async function load(data, progress) {
    loadData = data;
    device = data.device;
    try { embed = await create(loadData, device, progress); }
    catch (err) { fallback(err); embed = await create(loadData, device, progress); }
}
self.onmessage = async ({ data }) => {
    const { id, type } = data;
    try {
        if (type === 'load') {
            await load(data, (p) => { if (p.status === 'progress') self.postMessage({ id, progress: { file: p.file, loaded: p.loaded, total: p.total } }); });
            self.postMessage({ id, ok: true, device });
            return;
        }
        let out;
        try { out = await embed(data.texts); }
        catch (err) { fallback(err); embed = await create(loadData, device); out = await embed(data.texts); }
        const flat = new Float32Array(out.data);
        self.postMessage({ id, ok: true, device, dims: out.dims[out.dims.length - 1], data: flat }, [flat.buffer]);
    } catch (err) { self.postMessage({ id, error: String((err && err.message) || err) }); }
};`;

let worker: Worker | null = null;
// The model that the worker holds or loads. A failed load clears it, so the next call loads again.
let loaded: { key: ModelKey; ready: Promise<void> } | null = null;
let seq = 0;
// Requests to the worker that wait for their answer, by message id.
const pending = new Map<
    number,
    {
        resolve: (v: { dims: number; data: Float32Array; device?: string }) => void;
        reject: (e: Error) => void;
        progress?: (p: { file: string; loaded: number; total: number }) => void;
    }
>();

/**
 * Sends one request to the worker and resolves with its answer. Creates the worker on first use.
 * Rejects with the error text of the worker, or when `disable` ends the worker.
 */
function call(msg: Record<string, unknown>, progress?: (p: { file: string; loaded: number; total: number }) => void) {
    worker ??= Object.assign(
        new Worker(URL.createObjectURL(new Blob([WORKER], { type: 'text/javascript' })), {
            type: 'module',
            name: 'mars-smart-search',
        }),
        {
            onmessage: ({ data }: MessageEvent) => {
                const p = pending.get(data.id);
                if (!p) return;
                if (data.progress) {
                    p.progress?.(data.progress);
                    return;
                }
                pending.delete(data.id);
                if (data.error) p.reject(new Error(data.error));
                else p.resolve(data);
            },
        },
    );
    const id = ++seq;
    return new Promise<{ dims: number; data: Float32Array; device?: string }>((resolve, reject) => {
        pending.set(id, { resolve, reject, progress });
        worker!.postMessage({ id, ...msg });
    });
}

// A hardware WebGPU adapter. `navigator.gpu` can exist without one (VMs, blocklisted GPUs),
// and headless browsers offer a software fallback adapter, far too slow for embeddings.
let gpu: Promise<Gpu> | null = null;
/** Detects the WebGPU capability once for the page. Never rejects: a failure reports no GPU. */
export function gpuInfo() {
    type Adapter = {
        isFallbackAdapter?: boolean;
        info?: { isFallbackAdapter?: boolean };
        features?: Set<string>;
    } | null;
    const nav = navigator as unknown as { gpu?: { requestAdapter(): Promise<Adapter> } };
    gpu ??= (nav.gpu?.requestAdapter() ?? Promise.resolve(null))
        .then((a) => {
            const ok = !!a && !a.isFallbackAdapter && !a.info?.isFallbackAdapter;
            return { ok, f16: ok && !!a!.features?.has('shader-f16') };
        })
        .catch(() => ({ ok: false, f16: false }));
    return gpu;
}
/** This browser's device and weights for a model (see `placement`), or null when it cannot run here. */
export const placementOf = async (key: ModelKey) => placement(MODELS[key], await gpuInfo());

// The device the loaded model runs on: 'webgpu' or 'wasm' (WebAssembly).
let device: string | null = null;
/** The device of the loaded model, or null before a model loaded. */
export const activeDevice = () => device;

/**
 * Loads a model in the worker and reports the download and preparation as status. Calls for the
 * loaded model share one promise. Rejects when the model cannot run in this browser or the load
 * fails. A later call then starts a fresh load.
 */
function loadModel(key: ModelKey) {
    if (loaded?.key === key) return loaded.ready;
    const m = MODELS[key],
        files = new Map<string, [number, number]>();
    let line: Status = null;
    // Queries can load the model while indexing is paused; keep the pause badge visible.
    const show = (s: Status) => {
        line = s;
        status(suspension() ?? s);
    };
    const preparing = (): Status => ({ state: 'prepare', label: 'Preparing', text: `Preparing the ${m.label} model…` });
    // The loading status goes once the model is ready or failed; a status set meanwhile stays.
    // Re-emitting also refreshes the button's device label.
    const settle = () => status(currentStatus === line ? null : currentStatus);
    show(preparing());
    const ready = placementOf(key)
        .then((p) => {
            // EmbeddingGemma on WebAssembly takes minutes per document and can run out of memory.
            if (!p)
                throw new Error(
                    `the ${m.label} model needs a graphics card (WebGPU), which this browser doesn't offer. Pick another model from the sparkles button.`,
                );
            return call(
                {
                    type: 'load',
                    model: m.id,
                    file: m.file,
                    dtype: p.dtype,
                    cpuDtype: m.dtype,
                    device: p.device,
                    gpuOnly: m.gpu,
                },
                ({ file, loaded: got, total }) => {
                    // Sum the progress of all files. The denominator is at least the expected download
                    // size, so small files that finish first do not show a high percentage.
                    files.set(file, [got, total]);
                    const [done, all] = [...files.values()].reduce((s, [a, b]) => [s[0] + a, s[1] + b], [0, 0]);
                    const pct = Math.min(99, Math.round((done / Math.max(all, p.bytes)) * 100));
                    if (all > 1e6 && done < all)
                        show({
                            state: 'download',
                            label: `Downloading ${pct}%`,
                            text: `Downloading the ${m.label} model… ${pct}%`,
                        });
                    else if (all > 1e6 && line?.state !== 'prepare') show(preparing());
                },
            );
        })
        .then((r) => {
            device = r.device ?? null;
            settle();
        });
    loaded = { key, ready };
    ready.catch(() => {
        settle();
        if (loaded?.ready === ready) loaded = null;
    });
    return ready;
}

/** Embeds a batch of model inputs and returns one unit vector for each. Loads the model when needed. */
async function embed(key: ModelKey, texts: string[]) {
    await loadModel(key);
    const { dims, data, device: used } = await call({ type: 'embed', texts });
    // The worker can fall back from WebGPU to WebAssembly on first use. Follow its device.
    if (used && used !== device) {
        device = used;
        status(currentStatus);
    }
    return toVectors(MODELS[key], dims, data, texts.length);
}

// ---- Index ----
// Bumped by a model switch, a rebuild, and `disable`. A pass from an earlier generation stops
// and writes no entry.
let generation = 0;
// The pass in progress, or null.
let indexing: Promise<void> | null = null;
// A change arrived during a run, so another run follows it.
let queued = false;
/** Passage count per inference batch. Keep model inputs and batch sizes unchanged. */
const INDEX_BATCH_SIZE = 8;
// Bounded to one unfinished document per tab. No partial entry is written to vec:*.
let partial: {
    id: string;
    key: ModelKey;
    gen: number;
    storage: number;
    input: string;
    vectors: Float32Array[];
} | null = null;
// Yield a task, not only a microtask, so input and visibility events can run before the next batch.
const yieldToMain = () => {
    const scheduler = (window as unknown as { scheduler?: { yield?: () => Promise<void> } }).scheduler;
    return typeof scheduler?.yield === 'function'
        ? scheduler.yield()
        : new Promise<void>((resolve) => setTimeout(resolve, 0));
};
// Bumped after a run that changed the index, so earlier search results go stale.
let revision = 0;
/** A counter that changes when a pass changed the index. Search results belong to one revision. */
export const indexRevision = () => revision;
const indexedListeners = new Set<() => void>();
/** Registers a listener that runs after each pass that changed the index. */
export const onIndexed = (fn: () => void) => {
    indexedListeners.add(fn);
};
export const isPaused = () => paused;
/** Sets the pause state of this tab and its status. Returns true when the state changed. */
function setPaused(value: boolean) {
    if (paused === value) return false;
    paused = value;
    status(enabledModel() ? suspension() : null);
    return true;
}
/** Pauses indexing in every MARS tab after its current batch; retains unfinished vectors in this tab. */
export function pause() {
    try {
        localStorage.setItem(PAUSE_KEY, '1');
    } catch {}
    setPaused(true);
}
/** Ends the pause in every MARS tab and starts a pass for the documents that changed meanwhile. */
export function resume() {
    try {
        localStorage.removeItem(PAUSE_KEY);
    } catch {}
    setPaused(false);
    return reindex();
}
// Read the latest value, not a queued event's value: a newer toggle may already have won.
const syncPaused = () => {
    if (setPaused(readPaused()) && !paused) reindex();
};
window.addEventListener('storage', (e) => {
    if (e.storageArea === localStorage && (e.key === PAUSE_KEY || e.key === null)) syncPaused();
});
window.addEventListener('pageshow', syncPaused);
document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
        if (indexing) status(suspension());
    } else {
        syncPaused();
        reindex();
    }
});

// The storage generation and the model that the in-memory index was loaded for.
let indexStorage = -1,
    indexModel: ModelKey | null = null;
/**
 * Fills the in-memory index from storage, once for each storage backend and model. Entries of
 * another model or another index version are ignored, so the next pass replaces them.
 */
async function loadIndex(key: ModelKey) {
    const storage = db.storageGeneration();
    if (indexStorage === storage && indexModel === key) return;
    const entries = await db.allSides<Entry>('vec');
    // The storage backend changed during the read. These entries belong to the old backend.
    if (storage !== db.storageGeneration()) return loadIndex(key);
    index.clear();
    for (const e of entries) if (e?.model === key && e.v === indexVersion(key)) index.set(e.id, e);
    indexStorage = storage;
    indexModel = key;
}

/**
 * Embeds documents whose text changed since they were indexed; one run at a time.
 *
 * Does nothing when smart search is off, paused, or the tab is hidden. A call during a run queues one more run and
 * returns the promise of the current one. The promise never rejects: a failure becomes the
 * `error` status. A run drops entries of deleted documents, skips documents that were deleted
 * since it started, and stops between batches at suspension or a generation change.
 */
export function reindex() {
    const key = enabledModel();
    if (!key) return Promise.resolve();
    const suspended = suspension();
    if (suspended) {
        status(suspended);
        return Promise.resolve();
    }
    if (indexing) {
        queued = true;
        return indexing;
    }
    queued = false;
    const gen = generation;
    let changed = false;
    indexing = (async () => {
        try {
            await loadIndex(key);
            const storage = db.storageGeneration();
            const docs = app.state.nodes.filter((n) => n.kind === 'doc');
            const ids = new Set(docs.map((n) => n.id));
            if (
                partial &&
                (!ids.has(partial.id) || partial.gen !== gen || partial.storage !== storage || partial.key !== key)
            )
                partial = null;
            for (const id of index.keys())
                if (!ids.has(id)) {
                    index.delete(id);
                    changed = true;
                }
            const current = (e?: Entry) => e?.model === key && e.v === indexVersion(key);
            // Finish retained work first, so another stale document cannot replace the one-document buffer.
            const stale = docs
                .filter((n) => index.get(n.id)?.at !== n.updatedAt || !current(index.get(n.id)))
                .sort((a, b) => Number(b.id === partial?.id) - Number(a.id === partial?.id));
            for (const [i, n] of stale.entries()) {
                if (gen !== generation) return;
                if (suspension()) {
                    status(suspension());
                    return;
                }
                if (!app.node(n.id)) continue;
                const run = async () => {
                    const valid = () =>
                        gen === generation && enabledModel() === key && storage === db.storageGeneration();
                    if (!valid() || suspension()) return;
                    const snapshot = await db.getDocument(n.id);
                    if (!snapshot) return;
                    const { node, text } = snapshot;
                    const chunks = indexChunks(text);
                    const input = await inputFingerprint(MODELS[key], node.name, chunks);
                    // Recheck shared storage *inside* the lock. Another tab may already have done the inference.
                    const cached = await db.getSide<Entry>('vec', node.id);
                    const reuse = current(cached) && cached!.input === input && cached!.chunks.length === chunks.length;
                    // A resumed snapshot may have changed. Exact model inputs decide whether partial vectors still apply.
                    if (partial?.id === node.id && partial.input !== input) partial = null;
                    const snapshotCurrent = async () => {
                        if (!valid()) return false;
                        const latest = await db.getDocument(node.id);
                        if (!valid()) return false;
                        if (!latest) {
                            if (partial?.id === node.id) partial = null;
                            return false;
                        }
                        if (
                            latest.node.updatedAt !== node.updatedAt ||
                            latest.node.name !== node.name ||
                            latest.text !== text
                        ) {
                            if (partial?.id === node.id) partial = null;
                            queued = true;
                            return false;
                        }
                        return true;
                    };
                    let vectors: Float32Array[];
                    if (reuse) vectors = cached!.chunks.map((chunk) => chunk.v);
                    else {
                        if (!valid() || suspension()) return;
                        await loadModel(key);
                        if (!valid() || suspension()) return;
                        status({
                            state: 'index',
                            label: `Indexing ${i + 1}/${stale.length}`,
                            text: `Indexing ${i + 1} of ${stale.length}…`,
                        });
                        const work =
                            partial?.id === node.id ? partial : { id: node.id, key, gen, storage, input, vectors: [] };
                        partial = work;
                        vectors = work.vectors;
                        for (let j = vectors.length; j < chunks.length; j += INDEX_BATCH_SIZE) {
                            if (!valid()) {
                                if (partial === work) partial = null;
                                return;
                            }
                            if (suspension()) {
                                status(suspension());
                                return;
                            }
                            if (!(await snapshotCurrent())) return;
                            // The storage read itself can give a Pause or visibility event time to arrive.
                            if (suspension()) {
                                status(suspension());
                                return;
                            }
                            vectors.push(
                                ...(await embed(
                                    key,
                                    chunks
                                        .slice(j, j + INDEX_BATCH_SIZE)
                                        .map((c) => passageInput(MODELS[key], node.name, c)),
                                )),
                            );
                            await yieldToMain();
                        }
                    }
                    if (!(await snapshotCurrent())) return;
                    const entry: Entry = {
                        id: node.id,
                        model: key,
                        v: indexVersion(key),
                        at: node.updatedAt,
                        input,
                        chunks: chunks.map((c, j) => ({ start: c.start, text: c.text.slice(0, 220), v: vectors[j] })),
                    };
                    index.set(node.id, entry);
                    changed = true;
                    if (
                        !reuse ||
                        cached!.at !== entry.at ||
                        JSON.stringify(cached!.chunks.map((c) => [c.start, c.text])) !==
                            JSON.stringify(entry.chunks.map((c) => [c.start, c.text]))
                    )
                        await db.putSide('vec', node.id, entry);
                    if (partial?.id === node.id) partial = null;
                };
                if (!db.isTemporaryStorage() && navigator.locks)
                    await navigator.locks.request(`mars:index:${n.id}`, run);
                else await run();
            }
            status(suspension());
        } catch (err) {
            if (gen === generation) status(unavailable(err));
        } finally {
            indexing = null;
            if (
                partial &&
                (partial.gen !== generation ||
                    partial.storage !== db.storageGeneration() ||
                    partial.key !== enabledModel())
            )
                partial = null;
            if (changed) {
                revision++;
                for (const fn of indexedListeners) fn();
            }
            // A model switch or rebuild starts its own run once the old index is cleared.
            if (queued && gen === generation && !suspension()) reindex();
        }
    })();
    return indexing;
}

/**
 * The best-matching passage of each document, best first.
 * Returns at most `limit` documents whose score reaches the minimum of the model, and an empty
 * list when smart search is off. Loads the model when needed, also during a pause. Rejects when
 * the model cannot load or the worker fails.
 */
export async function search(query: string, limit = 6): Promise<Hit[]> {
    const key = enabledModel();
    if (!key) return [];
    await loadIndex(key);
    const [q] = await embed(key, [queryInput(MODELS[key], query)]);
    const live = [...index.values()].filter((e) => e.model === key && e.v === indexVersion(key) && app.node(e.id));
    return rank(live, q, MODELS[key].min, limit);
}

// ---- Downloaded models ----
// Vector notifications refresh search without replacing tree nodes or scheduling inference.
db.onSideChange((kind) => {
    if (kind !== 'vec') return;
    indexStorage = -1;
    revision++;
    for (const fn of indexedListeners) fn();
});

// Transformers.js keeps model files in Cache Storage, keyed by their Hugging Face URL.
const MODEL_CACHE = 'transformers-cache';
/** The model that a cached file URL belongs to, by its repository id in the path. */
const modelOf = (url: string) => (Object.keys(MODELS) as ModelKey[]).find((k) => url.includes(`/${MODELS[k].id}/`));

/** Bytes of each downloaded model in the browser cache. A model without cached files has no key. */
export async function modelUsage() {
    const out: Partial<Record<ModelKey, number>> = {};
    if (!('caches' in globalThis)) return out;
    const cache = await caches.open(MODEL_CACHE);
    for (const req of await cache.keys()) {
        const key = modelOf(req.url);
        if (!key) continue;
        const res = await cache.match(req);
        const bytes = Number(res?.headers.get('content-length')) || (await res?.blob())?.size || 0;
        out[key] = (out[key] ?? 0) + bytes;
    }
    return out;
}

/**
 * Whether the weights this browser would load for a model are in the cache. Cache Storage
 * holds whole files only, so the weights file plus most of the expected size means done.
 */
export async function isDownloaded(key: ModelKey) {
    const p = await placementOf(key);
    if (!p || !('caches' in globalThis)) return false;
    const weights = `/${MODELS[key].id}/resolve/main/onnx/${onnxFile(MODELS[key], p.dtype)}`;
    const urls = (await (await caches.open(MODEL_CACHE)).keys()).map((r) => r.url);
    return urls.some((u) => u.endsWith(weights)) && ((await modelUsage())[key] ?? 0) >= p.bytes * 0.9;
}

/** Deletes the cached files of one model. The next load downloads them again. */
export async function removeModel(key: ModelKey) {
    if (!('caches' in globalThis)) return;
    const cache = await caches.open(MODEL_CACHE);
    for (const req of await cache.keys()) if (req.url.includes(`/${MODELS[key].id}/`)) await cache.delete(req);
}

// ---- Settings ----
/**
 * Turns smart search on with a model and starts a pass.
 * `force` rebuilds the index even when the model stays the same.
 * A model change or a rebuild waits for the running pass to stop and then deletes every stored
 * entry. The pause state stays as it is.
 */
export async function enable(key: ModelKey, force = false) {
    const previous = enabledModel();
    try {
        localStorage.setItem(SETTING_KEY, key);
    } catch {}
    if (previous !== key || force) {
        generation++;
        partial = null;
        index.clear();
        indexStorage = -1;
        indexModel = null;
        await indexing;
        await db.clearSides('vec');
    }
    reindex();
}

/**
 * Turns smart search off and removes the index and every downloaded model.
 * Also clears the pause, ends the worker, and rejects its open requests.
 */
export async function disable() {
    try {
        localStorage.removeItem(SETTING_KEY);
    } catch {}
    try {
        localStorage.removeItem(PAUSE_KEY);
    } catch {}
    paused = false;
    generation++;
    partial = null;
    index.clear();
    indexStorage = -1;
    indexModel = null;
    worker?.terminate();
    worker = null;
    loaded = null;
    for (const p of pending.values()) p.reject(new Error('Smart search was turned off'));
    pending.clear();
    status(null);
    await db.clearSides('vec');
    for (const key of Object.keys(MODELS) as ModelKey[]) await removeModel(key);
}

// Tree changes schedule one pass, 2 seconds after the last change to a document timestamp or
// name. `scheduledDocs` is the document state that the pending pass was scheduled for.
let timer: ReturnType<typeof setTimeout> | undefined;
let scheduledDocs = new Map<string, { at: number; name: string }>();
app.onChange(() => {
    if (!enabledModel()) return;
    const docs = app.state.nodes.filter((node) => node.kind === 'doc');
    if (
        scheduledDocs.size === docs.length &&
        docs.every(
            (node) =>
                scheduledDocs.get(node.id)?.at === node.updatedAt && scheduledDocs.get(node.id)?.name === node.name,
        )
    )
        return;
    // Connection status and other presentation-only notifications must not keep
    // postponing a document change while multiple synced tabs are open.
    scheduledDocs = new Map(docs.map((node) => [node.id, { at: node.updatedAt, name: node.name }]));
    clearTimeout(timer);
    timer = setTimeout(reindex, 2000);
});
