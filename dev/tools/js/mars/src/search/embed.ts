// Smart (semantic) search: an embedding model runs in a module worker via Transformers.js,
// lazy-loaded from jsDelivr, with weights from Hugging Face cached by the browser. The index
// (one `vec:<id>` entry per document) lives in IndexedDB and follows document edits.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { createEncoder, type Gpu, type Hit, indexChunks, MODELS, type ModelKey, onnxFile, passageInput, placement, queryInput, rank, toVectors, TRANSFORMERS_VERSION } from './models.ts';

export { MODELS, type Hit, type ModelKey };

const TRANSFORMERS_URL = `https://cdn.jsdelivr.net/npm/@huggingface/transformers@${TRANSFORMERS_VERSION}`;

// Bump when what gets embedded, or how, changes, so stored indexes rebuild. A new model or
// quantization for a key rebuilds too.
const INDEX_VERSION = 5;
const indexVersion = (key: ModelKey) => `${INDEX_VERSION}:${MODELS[key].id}:${MODELS[key].dtype}`;
const SETTING_KEY = 'mars.smartSearch';
export function enabledModel(): ModelKey | null {
    try {
        const v = localStorage.getItem(SETTING_KEY);
        return v && v in MODELS ? (v as ModelKey) : null;
    } catch { return null; }
}

interface Entry { id: string; model: ModelKey; v?: number | string; at: number; chunks: { start: number; text: string; v: Float32Array }[] }

const index = new Map<string, Entry>();
// What smart search is doing: `label` fits the sidebar badge, `text` is the full sentence.
export type Status = { state: 'download' | 'prepare' | 'index' | 'paused' | 'error'; label: string; text: string } | null;
const statusListeners = new Set<(s: Status) => void>();
export const onStatus = (fn: (s: Status) => void) => { statusListeners.add(fn); };
let currentStatus: Status = null;
const status = (s: Status) => { currentStatus = s; for (const fn of statusListeners) fn(s); };
const PAUSED: Status = { state: 'paused', label: 'Paused', text: 'Smart-search indexing is paused' };
const unavailable = (err: unknown): Status => ({ state: 'error', label: 'Unavailable', text: `Smart search is unavailable: ${(err as Error)?.message ?? err}` });

// ---- Worker ----
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
let loaded: { key: ModelKey; ready: Promise<void> } | null = null;
let seq = 0;
const pending = new Map<number, { resolve: (v: { dims: number; data: Float32Array; device?: string }) => void; reject: (e: Error) => void; progress?: (p: { file: string; loaded: number; total: number }) => void }>();

function call(msg: Record<string, unknown>, progress?: (p: { file: string; loaded: number; total: number }) => void) {
    worker ??= Object.assign(new Worker(URL.createObjectURL(new Blob([WORKER], { type: 'text/javascript' })), { type: 'module', name: 'mars-smart-search' }), {
        onmessage: ({ data }: MessageEvent) => {
            const p = pending.get(data.id);
            if (!p) return;
            if (data.progress) { p.progress?.(data.progress); return; }
            pending.delete(data.id);
            if (data.error) p.reject(new Error(data.error)); else p.resolve(data);
        },
    });
    const id = ++seq;
    return new Promise<{ dims: number; data: Float32Array; device?: string }>((resolve, reject) => {
        pending.set(id, { resolve, reject, progress });
        worker!.postMessage({ id, ...msg });
    });
}

// A hardware WebGPU adapter. `navigator.gpu` can exist without one (VMs, blocklisted GPUs),
// and headless browsers offer a software fallback adapter, far too slow for embeddings.
let gpu: Promise<Gpu> | null = null;
export function gpuInfo() {
    type Adapter = { isFallbackAdapter?: boolean; info?: { isFallbackAdapter?: boolean }; features?: Set<string> } | null;
    const nav = navigator as Navigator & { gpu?: { requestAdapter(): Promise<Adapter> } };
    gpu ??= (nav.gpu?.requestAdapter() ?? Promise.resolve(null))
        .then((a) => {
            const ok = !!a && !a.isFallbackAdapter && !a.info?.isFallbackAdapter;
            return { ok, f16: ok && !!a!.features?.has('shader-f16') };
        })
        .catch(() => ({ ok: false, f16: false }));
    return gpu;
}
// This browser's device and weights for a model (see `placement`).
export const placementOf = async (key: ModelKey) => placement(MODELS[key], await gpuInfo());

// The device the loaded model runs on: 'webgpu' or 'wasm' (WebAssembly).
let device: string | null = null;
export const activeDevice = () => device;

function loadModel(key: ModelKey) {
    if (loaded?.key === key) return loaded.ready;
    const m = MODELS[key], files = new Map<string, [number, number]>();
    let line: Status = null;
    const show = (s: Status) => { line = s; status(s); };
    const preparing = (): Status => ({ state: 'prepare', label: 'Preparing', text: `Preparing the ${m.label} model…` });
    // The loading status goes once the model is ready or failed; a status set meanwhile stays.
    // Re-emitting also refreshes the button's device label.
    const settle = () => status(currentStatus === line ? null : currentStatus);
    show(preparing());
    const ready = placementOf(key).then((p) => {
        // EmbeddingGemma on WebAssembly takes minutes per document and can run out of memory.
        if (!p) throw new Error(`the ${m.label} model needs a graphics card (WebGPU), which this browser doesn't offer. Pick another model from the sparkles button.`);
        return call({ type: 'load', model: m.id, file: m.file, dtype: p.dtype, cpuDtype: m.dtype, device: p.device, gpuOnly: m.gpu }, ({ file, loaded: got, total }) => {
            files.set(file, [got, total]);
            const [done, all] = [...files.values()].reduce((s, [a, b]) => [s[0] + a, s[1] + b], [0, 0]);
            const pct = Math.min(99, Math.round((done / Math.max(all, p.bytes)) * 100));
            if (all > 1e6 && done < all) show({ state: 'download', label: `Downloading ${pct}%`, text: `Downloading the ${m.label} model… ${pct}%` });
            else if (all > 1e6 && line?.state !== 'prepare') show(preparing());
        });
    }).then((r) => { device = r.device ?? null; settle(); });
    loaded = { key, ready };
    ready.catch(() => { settle(); if (loaded?.ready === ready) loaded = null; });
    return ready;
}

async function embed(key: ModelKey, texts: string[]) {
    await loadModel(key);
    const { dims, data, device: used } = await call({ type: 'embed', texts });
    if (used && used !== device) { device = used; status(currentStatus); }
    return toVectors(MODELS[key], dims, data, texts.length);
}

// ---- Index ----
let generation = 0;
let indexing: Promise<void> | null = null;
// A change arrived during a run, so another run follows it.
let queued = false;
// Bumped after a run that changed the index, so earlier search results go stale.
let revision = 0;
export const indexRevision = () => revision;
const indexedListeners = new Set<() => void>();
export const onIndexed = (fn: () => void) => { indexedListeners.add(fn); };
// In memory only: a reload resumes indexing, so a forgotten pause can't leave the index stale.
let paused = false;
export const isPaused = () => paused;
export function pause() {
    paused = true;
    if (!indexing) status(PAUSED);
}
export function resume() {
    paused = false;
    status(null);
    return reindex();
}

async function loadIndex(key: ModelKey) {
    if (index.size) return;
    for (const e of await db.allSides<Entry>('vec')) if (e?.model === key && e.v === indexVersion(key)) index.set(e.id, e);
}

// Embeds documents whose text changed since they were indexed; one run at a time.
export function reindex() {
    const key = enabledModel();
    if (!key || paused) return Promise.resolve();
    if (indexing) { queued = true; return indexing; }
    queued = false;
    const gen = generation;
    let changed = false;
    indexing = (async () => {
        try {
            await loadIndex(key);
            const docs = app.state.nodes.filter((n) => n.kind === 'doc');
            const ids = new Set(docs.map((n) => n.id));
            for (const id of index.keys()) if (!ids.has(id)) { index.delete(id); changed = true; }
            const current = (e?: Entry) => e?.model === key && e.v === indexVersion(key);
            const stale = docs.filter((n) => index.get(n.id)?.at !== n.updatedAt || !current(index.get(n.id)));
            // Load first, so the status shows the download before "Indexing".
            if (stale.length) await loadModel(key);
            for (const [i, n] of stale.entries()) {
                if (gen !== generation) return;
                if (paused) { status(PAUSED); return; }
                if (!app.node(n.id)) continue;
                status({ state: 'index', label: `Indexing ${i + 1}/${stale.length}`, text: `Indexing ${i + 1} of ${stale.length}…` });
                const chunks = indexChunks((await db.getText(n.id)) ?? '');
                const vectors: Float32Array[] = [];
                for (let j = 0; j < chunks.length; j += 8) vectors.push(...(await embed(key, chunks.slice(j, j + 8).map((c) => passageInput(MODELS[key], n.name, c)))));
                if (gen !== generation || !app.node(n.id)) continue;
                const entry: Entry = { id: n.id, model: key, v: indexVersion(key), at: n.updatedAt, chunks: chunks.map((c, j) => ({ start: c.start, text: c.text.slice(0, 220), v: vectors[j] })) };
                index.set(n.id, entry);
                changed = true;
                await db.putSide('vec', n.id, entry);
            }
            status(paused ? PAUSED : null);
        } catch (err) {
            if (gen === generation) status(unavailable(err));
        } finally {
            indexing = null;
            if (changed) { revision++; for (const fn of indexedListeners) fn(); }
            // A model switch or rebuild starts its own run once the old index is cleared.
            if (queued && gen === generation && !paused) reindex();
        }
    })();
    return indexing;
}

// The best-matching passage of each document, best first.
export async function search(query: string, limit = 6): Promise<Hit[]> {
    const key = enabledModel();
    if (!key) return [];
    await loadIndex(key);
    const [q] = await embed(key, [queryInput(MODELS[key], query)]);
    const live = [...index.values()].filter((e) => e.model === key && e.v === indexVersion(key) && app.node(e.id));
    return rank(live, q, MODELS[key].min, limit);
}

// ---- Downloaded models ----
// Transformers.js keeps model files in Cache Storage, keyed by their Hugging Face URL.
const MODEL_CACHE = 'transformers-cache';
const modelOf = (url: string) => (Object.keys(MODELS) as ModelKey[]).find((k) => url.includes(`/${MODELS[k].id}/`));

// Bytes of each downloaded model in the browser cache.
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

// Whether the weights this browser would load for a model are in the cache. Cache Storage
// holds whole files only, so the weights file plus most of the expected size means done.
export async function isDownloaded(key: ModelKey) {
    const p = await placementOf(key);
    if (!p || !('caches' in globalThis)) return false;
    const weights = `/${MODELS[key].id}/resolve/main/onnx/${onnxFile(MODELS[key], p.dtype)}`;
    const urls = (await (await caches.open(MODEL_CACHE)).keys()).map((r) => r.url);
    return urls.some((u) => u.endsWith(weights)) && ((await modelUsage())[key] ?? 0) >= p.bytes * 0.9;
}

export async function removeModel(key: ModelKey) {
    if (!('caches' in globalThis)) return;
    const cache = await caches.open(MODEL_CACHE);
    for (const req of await cache.keys()) if (req.url.includes(`/${MODELS[key].id}/`)) await cache.delete(req);
}

// ---- Settings ----
// `force` rebuilds the index even when the model stays the same.
export async function enable(key: ModelKey, force = false) {
    const previous = enabledModel();
    paused = false;
    try { localStorage.setItem(SETTING_KEY, key); } catch {}
    if (previous !== key || force) {
        generation++;
        index.clear();
        await indexing;
        await db.clearSides('vec');
    }
    reindex();
}

// Turns smart search off and removes the index and every downloaded model.
export async function disable() {
    try { localStorage.removeItem(SETTING_KEY); } catch {}
    paused = false;
    generation++;
    index.clear();
    worker?.terminate();
    worker = null;
    loaded = null;
    for (const p of pending.values()) p.reject(new Error('Smart search was turned off'));
    pending.clear();
    status(null);
    await db.clearSides('vec');
    for (const key of Object.keys(MODELS) as ModelKey[]) await removeModel(key);
}

let timer: ReturnType<typeof setTimeout> | undefined;
app.onChange(() => {
    if (!enabledModel()) return;
    clearTimeout(timer);
    timer = setTimeout(reindex, 2000);
});
