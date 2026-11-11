// Smart-search quality evaluation: indexes corpus/ with the app's own chunking, encoder, and
// ranking (../src/search/models.ts), runs queries.json, and reports ranking quality, what the
// reader would see after the model's cut-off, and speed. See ../README.md#evaluation.
//
//   node eval.ts [model[@dtype] ...] [--check] [--verbose]
//
// `model` is a key of MODELS (fast, best) or CANDIDATES; the default is each shipped model
// with its CPU and WebGPU weights.
// `--check` exits 1 when a model scores below its row in bars.json.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as tf from '@huggingface/transformers';
import { createEncoder, type Indexed, indexChunks, type Model, MODELS, onnxFile, passageInput, queryInput, rank, toVectors, TRANSFORMERS_VERSION } from '../src/search/models.ts';

const HERE = dirname(fileURLToPath(import.meta.url));
tf.env.cacheDir = process.env.MARS_EVAL_CACHE ?? join(HERE, '.cache');
if (tf.env.version !== TRANSFORMERS_VERSION) throw new Error(`eval/ has Transformers.js ${tf.env.version}; the app loads ${TRANSFORMERS_VERSION}. Align eval/package.json.`);

// Models worth comparing against the shipped ones; same fields as MODELS.
const base = { file: 'model', bytes: 0, langs: 'English', query: '', doc: () => '', dims: 0, gpu: false };
const CANDIDATES: Record<string, Model> = {
    minilm: { ...base, label: 'all-MiniLM-L6-v2', name: 'all-MiniLM-L6-v2', id: 'Xenova/all-MiniLM-L6-v2', dtype: 'q8', min: 0.3 },
    minilm12: { ...base, label: 'all-MiniLM-L12-v2', name: 'all-MiniLM-L12-v2', id: 'Xenova/all-MiniLM-L12-v2', dtype: 'q8', min: 0.3 },
    multiqa: { ...base, label: 'multi-qa-MiniLM-L6-cos-v1', name: 'multi-qa-MiniLM-L6-cos-v1', id: 'Xenova/multi-qa-MiniLM-L6-cos-v1', dtype: 'q8', min: 0.3 },
    gte: { ...base, label: 'gte-small', name: 'gte-small', id: 'Xenova/gte-small', dtype: 'q8', min: 0.81 },
};
const ALL: Record<string, Model> = { ...MODELS, ...CANDIDATES };

const SHOWN = 6; // results the sidebar shows

interface Query { q: string; expect: string[]; also?: string[] }
const { queries, offTopic } = JSON.parse(readFileSync(join(HERE, 'queries.json'), 'utf8')) as { queries: Query[]; offTopic: string[] };
const corpusDir = join(HERE, 'corpus');
const walk = (d: string): string[] => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : f.endsWith('.md') ? [join(d, f)] : []));
const corpus = walk(corpusDir).map((f) => ({ id: relative(corpusDir, f), title: basename(f, '.md'), text: readFileSync(f, 'utf8') }));
for (const q of queries) for (const id of [...q.expect, ...(q.also ?? [])]) if (!corpus.some((d) => d.id === id)) throw new Error(`queries.json: "${q.q}" names a missing document: ${id}`);
const isShort = (q: string) => q.split(/\s+/).length <= 3;

// Bytes of the files the model loads: its ONNX weights for the dtype plus tokenizer and config.
function downloadBytes(m: Model, d = join(tf.env.cacheDir!, m.id)): number {
    const stem = onnxFile(m, m.dtype);
    return readdirSync(d).reduce((s, f) => {
        const p = join(d, f);
        if (statSync(p).isDirectory()) return s + downloadBytes(m, p);
        return s + (basename(d) === 'onnx' && !f.startsWith(stem) ? 0 : statSync(p).size);
    }, 0);
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / (xs.length || 1);
function auc(pos: number[], neg: number[]) {
    let w = 0;
    for (const p of pos) for (const n of neg) w += p > n ? 1 : p === n ? 0.5 : 0;
    return w / (pos.length * neg.length);
}
// The cut-off with the best balanced accuracy: answers at or above it, off-topic tops below.
function bestCut(pos: number[], neg: number[]) {
    let best = { t: 0, acc: 0 };
    for (const t of [...pos, ...neg]) {
        const acc = (pos.filter((p) => p >= t).length / pos.length + neg.filter((n) => n < t).length / neg.length) / 2;
        if (acc > best.acc) best = { t, acc };
    }
    return best;
}

async function evaluate(spec: string, verbose: boolean) {
    const [key, dtype] = spec.split('@');
    if (!ALL[key]) throw new Error(`Unknown model "${key}"; pick one of ${Object.keys(ALL).join(', ')}`);
    const m: Model = { ...ALL[key], ...(dtype ? { dtype } : {}) };
    let t0 = performance.now();
    const encode = await createEncoder(tf, { model: m.id, file: m.file, dtype: m.dtype }, 'cpu');
    const loadMs = performance.now() - t0;
    const embed = async (texts: string[]) => { const out = await encode(texts); return toVectors(m, out.dims.at(-1)!, out.data as Float32Array, texts.length); };

    t0 = performance.now();
    const docs: Indexed[] = [];
    let chunkCount = 0;
    for (const d of corpus) {
        const chunks = indexChunks(d.text), vectors: Float32Array[] = [];
        for (let j = 0; j < chunks.length; j += 8) vectors.push(...(await embed(chunks.slice(j, j + 8).map((c) => passageInput(m, d.title, c)))));
        docs.push({ id: d.id, chunks: chunks.map((c, j) => ({ start: c.start, text: c.text, v: vectors[j] })) });
        chunkCount += chunks.length;
    }
    const indexMs = performance.now() - t0;

    t0 = performance.now();
    const rows = [];
    for (const query of queries) {
        const [qv] = await embed([queryInput(m, query.q)]);
        const all = rank(docs, qv);
        const also = new Set(query.also ?? []), want = new Set(query.expect);
        // `also` documents are fair to show but are not the answer: left out of rank metrics.
        const ranked = all.filter((h) => !also.has(h.id));
        const r = ranked.findIndex((h) => want.has(h.id)) + 1;
        const dcg = ranked.slice(0, 10).reduce((s, h, i) => s + (want.has(h.id) ? 1 / Math.log2(i + 2) : 0), 0);
        const ideal = Array.from({ length: Math.min(want.size, 10) }, (_, i) => 1 / Math.log2(i + 2)).reduce((a, b) => a + b, 0);
        const shown = all.filter((h) => h.score >= m.min).slice(0, SHOWN);
        rows.push({
            q: query.q, short: isShort(query.q), rank: r, rr: r && r <= 10 ? 1 / r : 0, ndcg: dcg / ideal,
            found: shown.some((h) => want.has(h.id)),
            precision: shown.length ? shown.filter((h) => want.has(h.id) || also.has(h.id)).length / shown.length : null,
            top: Math.max(...all.filter((h) => want.has(h.id)).map((h) => h.score)), shown,
        });
    }
    const off = [];
    for (const q of offTopic) {
        const [qv] = await embed([queryInput(m, q)]);
        const all = rank(docs, qv);
        off.push({ q, top: all[0].score, shown: all.filter((h) => h.score >= m.min).slice(0, SHOWN) });
    }
    const queryMs = (performance.now() - t0) / (queries.length + offTopic.length);

    if (verbose) {
        for (const r of rows) console.log(`  ${r.found ? '✓' : '✗'} rank ${String(r.rank || '-').padStart(2)}  ${r.top.toFixed(3)}  ${r.q}${r.shown.length ? `  →  ${r.shown.map((h) => `${basename(h.id, '.md')} ${h.score.toFixed(2)}`).join(', ')}` : ''}`);
        for (const o of off) console.log(`  ${o.shown.length ? '✗' : '✓'} off-topic ${o.top.toFixed(3)}  ${o.q}${o.shown.length ? `  →  ${o.shown.map((h) => basename(h.id, '.md')).join(', ')}` : ''}`);
    }
    const part = (f: (r: (typeof rows)[number]) => boolean) => rows.filter(f);
    const cut = bestCut(rows.map((r) => r.top), off.map((o) => o.top));
    return {
        spec: `${key}@${m.dtype}`, key, min: m.min,
        mrr: mean(rows.map((r) => r.rr)), ndcg: mean(rows.map((r) => r.ndcg)),
        hit1: mean(rows.map((r) => +(r.rank === 1))), hit3: mean(rows.map((r) => +(r.rank > 0 && r.rank <= 3))),
        mrrShort: mean(part((r) => r.short).map((r) => r.rr)), mrrLong: mean(part((r) => !r.short).map((r) => r.rr)),
        found: mean(rows.map((r) => +r.found)), precision: mean(rows.flatMap((r) => (r.precision === null ? [] : [r.precision]))),
        clean: mean(off.map((o) => +!o.shown.length)),
        auc: auc(rows.map((r) => r.top), off.map((o) => o.top)), cut: cut.t, cutAcc: cut.acc,
        mb: downloadBytes(m) / 1e6, loadS: loadMs / 1000, chunkMs: indexMs / chunkCount, queryMs,
    };
}

const args = process.argv.slice(2);
const specs = args.filter((a) => !a.startsWith('--'));
const results = [];
// By default, every shipped model with each weights it loads (CPU and WebGPU). Quality is
// measured on CPU; the weights are what differ.
const shipped = Object.entries(MODELS).flatMap(([k, m]: [string, Model]) => [`${k}@${m.dtype}`, ...(m.gpuDtype ? [`${k}@${m.gpuDtype}`] : [])]);
for (const spec of specs.length ? specs : shipped) {
    console.log(`\n${spec}`);
    results.push(await evaluate(spec, args.includes('--verbose')));
}

const pct = (x: number) => `${Math.round(x * 100)}%`;
const COLUMNS: [string, (r: (typeof results)[number]) => string][] = [
    ['Model', (r) => r.spec], ['MRR', (r) => r.mrr.toFixed(3)], ['short', (r) => r.mrrShort.toFixed(3)], ['long', (r) => r.mrrLong.toFixed(3)],
    ['nDCG@10', (r) => r.ndcg.toFixed(3)], ['Hit@1', (r) => pct(r.hit1)], ['Hit@3', (r) => pct(r.hit3)],
    ['Found', (r) => pct(r.found)], ['Precision', (r) => pct(r.precision)], ['Clean', (r) => pct(r.clean)],
    ['AUC', (r) => r.auc.toFixed(3)], ['min', (r) => r.min.toFixed(2)], ['Best cut', (r) => `${r.cut.toFixed(3)} (${pct(r.cutAcc)})`],
    ['MB', (r) => r.mb.toFixed(0)], ['Load s', (r) => r.loadS.toFixed(1)], ['ms/chunk', (r) => r.chunkMs.toFixed(1)], ['ms/query', (r) => r.queryMs.toFixed(1)],
];
console.log(`\n${corpus.length} documents, ${queries.length} queries (${queries.filter((q) => isShort(q.q)).length} short), ${offTopic.length} off-topic, Node CPU\n`);
console.log(`| ${COLUMNS.map(([h]) => h).join(' | ')} |\n|${COLUMNS.map(() => '---').join('|')}|`);
for (const r of results) console.log(`| ${COLUMNS.map(([, f]) => f(r)).join(' | ')} |`);

if (args.includes('--check')) {
    const bars = JSON.parse(readFileSync(join(HERE, 'bars.json'), 'utf8')) as Record<string, Record<string, number>>;
    let failed = false;
    for (const r of results) {
        const bar = bars[r.spec];
        if (!bar) { console.log(`\nFAIL ${r.spec}: no row in bars.json`); failed = true; continue; }
        for (const [metric, floor] of Object.entries(bar)) {
            const value = r[metric as keyof typeof r] as number;
            if (value < floor) { console.log(`\nFAIL ${r.spec}: ${metric} ${value.toFixed(3)} is below ${floor}`); failed = true; }
        }
    }
    if (failed) process.exit(1);
    console.log('\nAll models meet bars.json.');
}
