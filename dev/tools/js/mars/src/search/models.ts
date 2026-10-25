// Smart-search models and the pure parts of embedding and ranking, shared by the app
// (search/embed.ts) and the quality evaluation (eval/), so the evaluation measures what ships.
import { type Chunk, chunkDoc, embeddable, searchBody, threadChunks } from './chunks.ts';

// The Transformers.js release the app loads; eval/package.json pins the same version.
export const TRANSFORMERS_VERSION = '4.3.1';

export interface Model {
    label: string;
    name: string;
    id: string;
    // ONNX file stem (`onnx/<file>_<dtype>.onnx`), the quantization, and its download size.
    file: string;
    dtype: string;
    bytes: number;
    // The quantization for a WebGPU adapter with `shader-f16`, and its download size. On
    // WebGPU, float16 weights run far faster than 8-bit ones (see `placement`).
    gpuDtype?: string;
    gpuBytes?: number;
    langs: string;
    // Prefixes the model was trained with, for queries and for passages of a document.
    query: string;
    doc: (title: string) => string;
    // Matryoshka truncation: keep the leading `dims` values (0 keeps all).
    dims: number;
    // The lowest score (cosine similarity) worth showing. Each model has its own scale;
    // `npm run eval` in eval/ reports the cut-off that best separates answers from off-topic
    // queries. ponytail: one fixed cut-off per model.
    min: number;
    // WebGPU only: EmbeddingGemma's q4 file needs an operator (GatherBlockQuantized) that
    // WebAssembly lacks, and on WebAssembly it takes minutes per document anyway.
    gpu: boolean;
}

// Picked by the evaluation in eval/ (see js/mars/README.md). Licenses must allow any use
// (MARS is a public tool).
export const MODELS = {
    fast: { label: 'Best speed', name: 'GTE Small 33M', id: 'Xenova/gte-small', file: 'model', dtype: 'q8', bytes: 35e6, gpuDtype: 'fp16', gpuBytes: 68e6, langs: 'English', query: '', doc: () => '', dims: 0, min: 0.81, gpu: false },
    best: { label: 'Best quality', name: 'EmbeddingGemma 2 270M', id: 'onnx-community/embeddinggemma-2-ONNX', file: 'model', dtype: 'q4', bytes: 207e6, gpuDtype: 'q4f16', gpuBytes: 190e6, langs: '100+ languages', query: 'task: search result | query: ', doc: (title) => `title: ${title || 'none'} | text: `, dims: 256, min: 0.7, gpu: true },
} satisfies Record<string, Model>;
export type ModelKey = keyof typeof MODELS;

// The ONNX file a dtype loads, as Transformers.js names them.
const SUFFIX: Record<string, string> = { fp32: '', fp16: '_fp16', q8: '_quantized', int8: '_int8', uint8: '_uint8', q4: '_q4', q4f16: '_q4f16', bnb4: '_bnb4' };
export const onnxFile = (m: Model, dtype: string) => `${m.file}${SUFFIX[dtype] ?? `_${dtype}`}.onnx`;

export interface Gpu { ok: boolean; f16: boolean }
export interface Placement { device: 'webgpu' | 'wasm'; dtype: string; bytes: number }

// Where a model runs and which weights it loads, or null when it cannot run here. Per chunk
// in Chrome on an Apple M-series GPU, GTE Small takes 7 ms at fp16 on WebGPU, 78 ms at q8 on
// WebGPU, and 48 ms at q8 on WebAssembly threads (600 ms in Firefox); EmbeddingGemma 2 takes
// 23 ms at q4f16 and 39 ms at q4. Without `shader-f16`, Best speed stays on WebAssembly.
export function placement(m: Model, gpu: Gpu): Placement | null {
    if (gpu.ok && gpu.f16 && m.gpuDtype) return { device: 'webgpu', dtype: m.gpuDtype, bytes: m.gpuBytes ?? m.bytes };
    if (m.gpu) return gpu.ok ? { device: 'webgpu', dtype: m.dtype, bytes: m.bytes } : null;
    return { device: 'wasm', dtype: m.dtype, bytes: m.bytes };
}

// A long paragraph or code block can exceed the chunk target; the cap bounds a batch's
// token count (and memory).
export const MAX_EMBED_CHARS = 1500;

// The passages of a document that get a vector: paragraphs with their heading, then review
// threads of at least three words.
export function indexChunks(text: string): Chunk[] {
    const body = searchBody(text);
    return [...chunkDoc(body).filter(embeddable), ...threadChunks(text, body, 3)];
}

export const passageInput = (m: Model, title: string, c: Chunk) => m.doc(title) + (c.embed ?? c.text).slice(0, MAX_EMBED_CHARS);
export const queryInput = (m: Model, query: string) => m.query + query;

// Builds the embedding function from a Transformers.js module (`tf`). The worker runs this
// function's source text, so it may use only its parameters and globals.
export async function createEncoder(
    tf: any,
    data: { model: string; file: string; dtype: string },
    device: string,
    progress_callback?: (p: unknown) => void,
) {
    const config = await tf.AutoConfig.from_pretrained(data.model, { progress_callback });
    // Text only: EmbeddingGemma 2 is multimodal and would also load its vision and audio
    // encoders (300 MB at q4).
    config.vision_config = config.audio_config = null;
    const tokenizer = await tf.AutoTokenizer.from_pretrained(data.model, { progress_callback });
    const model = await tf.AutoModel.from_pretrained(data.model, { config, dtype: data.dtype, device, model_file_name: data.file, progress_callback });
    return async (texts: string[]): Promise<{ dims: number[]; data: Float32Array }> => {
        const inputs = await tokenizer(texts, { padding: true, truncation: true });
        const out = await model(inputs);
        // Models exported with their pooling (Granite, EmbeddingGemma) return it as
        // `sentence_embedding`; the rest are mean-pooled, as their model cards do.
        return out.sentence_embedding ?? tf.mean_pooling(out.last_hidden_state, inputs.attention_mask);
    };
}

// Splits a [count, dims] batch into unit vectors, truncated to `m.dims` first.
export function toVectors(m: Model, dims: number, data: Float32Array, count: number) {
    const keep = m.dims || dims;
    return Array.from({ length: count }, (_, i) => {
        const v = data.slice(i * dims, i * dims + keep);
        let norm = 0;
        for (const x of v) norm += x * x;
        norm = Math.sqrt(norm) || 1;
        for (let j = 0; j < v.length; j++) v[j] /= norm;
        return v;
    });
}

export interface Hit { id: string; score: number; start: number; text: string }
export interface Indexed { id: string; chunks: { start: number; text: string; v: Float32Array }[] }

// The best-matching passage of each document, best first: what the sidebar shows. With
// `min`, documents scoring lower are left out.
export function rank(docs: Iterable<Indexed>, q: Float32Array, min = -Infinity, limit = Infinity): Hit[] {
    const hits: Hit[] = [];
    for (const e of docs) {
        let best: Hit | null = null;
        for (const c of e.chunks) {
            if (c.v.length !== q.length) continue;
            let score = 0;
            for (let i = 0; i < q.length; i++) score += q[i] * c.v[i];
            if (!best || score > best.score) best = { id: e.id, score, start: c.start, text: c.text };
        }
        if (best && best.score >= min) hits.push(best);
    }
    return hits.sort((a, b) => b.score - a.score).slice(0, limit);
}
