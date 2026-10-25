// Smart (semantic) search: an embedding model runs in a module worker via Transformers.js,
// lazy-loaded from jsDelivr, with weights from Hugging Face cached by the browser. The index
// (one `vec:<id>` entry per document) lives in IndexedDB and follows document edits.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { chunkDoc, embeddable, searchBody, threadChunks } from './chunks.ts';

const TRANSFORMERS_URL = 'https://cdn.jsdelivr.net/npm/@huggingface/transformers@4.3.1';

// The models won a benchmark of seven on 70 real security-review reports (936 chunks, 30
// paraphrased queries with known answers, 10 off-topic queries); see js/mars/README.md.
// `min`: the lowest score worth showing (cosine similarity), the cut-off that best separated
// answers from off-topic queries there: Granite 95% at 0.79, EmbeddingGemma 2 98% at 0.70.
// Each model has its own scale. ponytail: fixed cut-offs; re-run the benchmark when
// changing a model or when results feel noisy or thin.
// `pooling`: how token states become the embedding ('cls' for Granite). `sentence`: the
// model's own pooled `sentence_embedding` output is the embedding instead.
// `gpu`: WebGPU only. EmbeddingGemma's q4 file needs an operator (GatherBlockQuantized)
// that WebAssembly lacks, and on WebAssembly it takes minutes per document anyway.
// Licenses must allow any use (MARS is a public tool): both are Apache-2.0.
export const MODELS = {
    fast: { label: 'Best speed', name: 'Granite Embedding Small English R2 47M', id: 'onnx-community/granite-embedding-small-english-r2-ONNX', file: 'model', dtype: 'q8', size: '55 MB', bytes: 55e6, langs: 'English', query: '', doc: () => '', dims: 0, min: 0.79, gpu: false, sentence: false, pooling: 'cls' },
    best: { label: 'Best quality', name: 'EmbeddingGemma 2 270M', id: 'onnx-community/embeddinggemma-2-ONNX', file: 'model', dtype: 'q4', size: '174 MB', bytes: 174e6, langs: '100+ languages', query: 'task: search result | query: ', doc: (title: string) => `title: ${title || 'none'} | text: `, dims: 256, min: 0.7, gpu: true, sentence: true, pooling: 'mean' },
} as const;
// Bump when what gets embedded, or how, changes, so stored indexes rebuild.
const INDEX_VERSION = 4;
const MAX_EMBED_CHARS = 1500;
export type ModelKey = keyof typeof MODELS;

// Models MARS used before: their downloaded files are removed once, and a browser that had
// the dropped "multi" model selected switches to Best speed.
const RETIRED = ['Xenova/multilingual-e5-small', 'Xenova/all-MiniLM-L6-v2', 'onnx-community/embeddinggemma-300m-ONNX'];

const SETTING_KEY = 'mars.smartSearch';
export function enabledModel(): ModelKey | null {
    try {
        let v = localStorage.getItem(SETTING_KEY);
        if (v === 'multi') localStorage.setItem(SETTING_KEY, (v = 'fast'));
        return v && v in MODELS ? (v as ModelKey) : null;
    } catch { return null; }
}

interface Entry { id: string; model: ModelKey; v?: number; at: number; chunks: { start: number; text: string; v: Float32Array }[] }
export interface Hit { id: string; score: number; start: number; text: string }

const index = new Map<string, Entry>();
const statusListeners = new Set<(s: string | null) => void>();
export const onStatus = (fn: (s: string | null) => void) => { statusListeners.add(fn); };
let currentStatus: string | null = null;
const status = (s: string | null) => { currentStatus = s; for (const fn of statusListeners) fn(s); };

// ---- Worker ----
const WORKER = `
import { pipeline, AutoTokenizer, AutoModel, env } from '${TRANSFORMERS_URL}';
env.allowLocalModels = false;
let embed;
// \`sentence\`: read the model's pooled sentence_embedding output; otherwise pool token states
// (\`pooling\`: 'mean' or 'cls'; the pipeline's 'last_token' ignores padding, so avoid it).
async function create(data, device, progress_callback) {
    const opts = { dtype: data.dtype, device, progress_callback, model_file_name: data.file };
    if (!data.sentence) {
        const extractor = await pipeline('feature-extraction', data.model, opts);
        return (texts) => extractor(texts, { pooling: data.pooling, normalize: true });
    }
    const tokenizer = await AutoTokenizer.from_pretrained(data.model, { progress_callback });
    const model = await AutoModel.from_pretrained(data.model, opts);
    return async (texts) => (await model(await tokenizer(texts, { padding: true, truncation: true }))).sentence_embedding;
}
// The main thread picks WebGPU only after hasGpu(). A WebGPU failure, at load or first use,
// falls back to WebAssembly unless the model is GPU-only (\`gpuOnly\`).
let loadData, device;
const fallback = (err) => { if (device === 'wasm' || loadData.gpuOnly) throw err; device = 'wasm'; };
async function load(data, progress) {
    loadData = data;
    device = data.device;
    try { embed = await create(data, device, progress); }
    catch (err) { fallback(err); embed = await create(data, device, progress); }
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
let gpu: Promise<boolean> | null = null;
export function hasGpu() {
    type Adapter = { isFallbackAdapter?: boolean; info?: { isFallbackAdapter?: boolean } } | null;
    const nav = navigator as Navigator & { gpu?: { requestAdapter(): Promise<Adapter> } };
    gpu ??= (nav.gpu?.requestAdapter() ?? Promise.resolve(null))
        .then((a) => !!a && !a.isFallbackAdapter && !a.info?.isFallbackAdapter)
        .catch(() => false);
    return gpu;
}

// The device the loaded model runs on: 'webgpu' or 'wasm' (WebAssembly).
let device: string | null = null;
export const activeDevice = () => device;

// Best quality runs only on WebGPU. Best speed runs on WebAssembly threads when the page is
// cross-origin isolated, and on WebGPU otherwise (Safari), when there is one. Indexing 936
// chunks (Apple M-series): Granite 124 s on threads, 480 s on one thread; EmbeddingGemma 2
// 77 s on WebGPU.
function loadModel(key: ModelKey) {
    if (loaded?.key === key) return loaded.ready;
    const m = MODELS[key], files = new Map<string, [number, number]>();
    status(`Preparing the ${m.label} model…`);
    const ready = hasGpu().then((gpuOk) => {
        // EmbeddingGemma on WebAssembly takes minutes per document and can run out of memory.
        if (m.gpu && !gpuOk) throw new Error(`the ${m.label} model needs a graphics card (WebGPU), which this browser doesn't offer. Pick another model from the sparkles button.`);
        const onGpu = m.gpu || (gpuOk && !crossOriginIsolated);
        return call({ type: 'load', model: m.id, file: m.file, dtype: m.dtype, device: onGpu ? 'webgpu' : 'wasm', gpuOnly: m.gpu, sentence: m.sentence, pooling: m.pooling }, ({ file, loaded: got, total }) => {
            files.set(file, [got, total]);
            const [done, all] = [...files.values()].reduce((s, [a, b]) => [s[0] + a, s[1] + b], [0, 0]);
            if (all > 1e6 && done < all) status(`Downloading the ${m.label} model… ${Math.min(99, Math.round((done / Math.max(all, m.bytes)) * 100))}%`);
            else if (all > 1e6) status(`Preparing the ${m.label} model…`);
        });
    }).then((r) => { device = r.device ?? null; status(currentStatus); });
    loaded = { key, ready };
    ready.catch(() => { if (loaded?.ready === ready) loaded = null; });
    return ready;
}

// Embeddings, normalized; EmbeddingGemma is truncated to `dims` (Matryoshka) first.
async function embed(key: ModelKey, texts: string[]) {
    await loadModel(key);
    const { dims, data, device: used } = await call({ type: 'embed', texts });
    if (used && used !== device) { device = used; status(currentStatus); }
    const keep = MODELS[key].dims || dims;
    return texts.map((_, i) => {
        const v = data.slice(i * dims, i * dims + keep);
        let norm = 0;
        for (const x of v) norm += x * x;
        norm = Math.sqrt(norm) || 1;
        for (let j = 0; j < v.length; j++) v[j] /= norm;
        return v;
    });
}

// ---- Index ----
let generation = 0;
let indexing: Promise<void> | null = null;

async function loadIndex(key: ModelKey) {
    if (index.size) return;
    for (const e of await db.allSides<Entry>('vec')) if (e?.model === key && e.v === INDEX_VERSION) index.set(e.id, e);
}

// Embeds documents whose text changed since they were indexed; one run at a time.
export function reindex() {
    const key = enabledModel();
    if (!key) return Promise.resolve();
    if (indexing) return indexing;
    const gen = generation;
    indexing = (async () => {
        try {
            await loadIndex(key);
            const docs = app.state.nodes.filter((n) => n.kind === 'doc');
            const ids = new Set(docs.map((n) => n.id));
            for (const id of index.keys()) if (!ids.has(id)) index.delete(id);
            const current = (e?: Entry) => e?.model === key && e.v === INDEX_VERSION;
            const stale = docs.filter((n) => index.get(n.id)?.at !== n.updatedAt || !current(index.get(n.id)));
            // Load first, so the status line shows the download before "Indexing".
            if (stale.length) await loadModel(key);
            for (const [i, n] of stale.entries()) {
                if (gen !== generation) return;
                status(`Indexing ${i + 1} of ${stale.length}…`);
                const text = (await db.getText(n.id)) ?? '', body = searchBody(text);
                const chunks = [...chunkDoc(body).filter(embeddable), ...threadChunks(text, body, 3)];
                const prefix = MODELS[key].doc(n.name);
                const vectors: Float32Array[] = [];
                // A single long paragraph or code block can exceed the chunk target; the cap
                // keeps a batch's token count (and memory) bounded.
                for (let j = 0; j < chunks.length; j += 8) vectors.push(...(await embed(key, chunks.slice(j, j + 8).map((c) => prefix + (c.embed ?? c.text).slice(0, MAX_EMBED_CHARS)))));
                if (gen !== generation) return;
                const entry: Entry = { id: n.id, model: key, v: INDEX_VERSION, at: n.updatedAt, chunks: chunks.map((c, j) => ({ start: c.start, text: c.text.slice(0, 220), v: vectors[j] })) };
                index.set(n.id, entry);
                if (app.node(n.id)) await db.putSide('vec', n.id, entry);
            }
            status(null);
        } catch (err) {
            if (gen === generation) status(`Smart search is unavailable: ${(err as Error)?.message ?? err}`);
        } finally {
            indexing = null;
        }
    })();
    return indexing;
}

// The best-matching passage of each document, best first.
export async function search(query: string, limit = 6): Promise<Hit[]> {
    const key = enabledModel();
    if (!key) return [];
    await loadIndex(key);
    const [q] = await embed(key, [MODELS[key].query + query]);
    const hits: Hit[] = [];
    for (const e of index.values()) {
        if (e.model !== key || e.v !== INDEX_VERSION || !app.node(e.id)) continue;
        let best: Hit | null = null;
        for (const c of e.chunks) {
            if (c.v.length !== q.length) continue;
            let score = 0;
            for (let i = 0; i < q.length; i++) score += q[i] * c.v[i];
            if (!best || score > best.score) best = { id: e.id, score, start: c.start, text: c.text };
        }
        if (best && best.score >= MODELS[key].min) hits.push(best);
    }
    return hits.sort((a, b) => b.score - a.score).slice(0, limit);
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

async function dropCached(id: string) {
    if (!('caches' in globalThis)) return;
    const cache = await caches.open(MODEL_CACHE);
    for (const req of await cache.keys()) if (req.url.includes(`/${id}/`)) await cache.delete(req);
}
export const removeModel = (key: ModelKey) => dropCached(MODELS[key].id);
// ponytail: checks the cache on every page load; drop this once old installs have cleaned up.
for (const id of RETIRED) dropCached(id).catch(() => {});

// ---- Settings ----
// `force` rebuilds the index even when the model stays the same.
export async function enable(key: ModelKey, force = false) {
    const previous = enabledModel();
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
