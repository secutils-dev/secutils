// Document viewer and editor, ported from dev/tools/md.html (its live-app script,
// minus HTML/PDF export, the HTML view, and URL-fragment autosave).
// ponytail: a copy, not a shared module, so md.html stays a dependency-free single
// file. Fixes to rendering, block editing, or comments may need to land in both.
// The host (main.ts) owns storage, routing, and sharing; see `setHost`.
import { marked } from 'marked';
import { loadEditor } from './editor.js';

let host = {};
export const setHost = (h) => { host = h; };
// MARS-only review-changes hooks (absent from md.html); see "Review changes hooks" below.
const renderedHooks = [];
const renderedHook = () => { for (const fn of renderedHooks) fn(); applyReveal(); };
let threadHooks = {};
let pendingReveal = null;

const MERMAID_URL = 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
const TOPOJSON_URL = 'https://esm.sh/topojson-client@3.1.0?bundle';
const KATEX_VERSION = '0.16.11';
const KATEX_CSS_URL = `https://cdn.jsdelivr.net/npm/katex@${KATEX_VERSION}/dist/katex.min.css`;
const KATEX_JS_URL = `https://cdn.jsdelivr.net/npm/katex@${KATEX_VERSION}/dist/katex.mjs`;
const MD_FILE_EXT = /\.(md|markdown|txt|mdx)$/i;
const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n/;
const FRONTMATTER_TITLE_RE = /^title:\s*['"]?(.+?)['"]?\s*$/m;

const $ = (id) => document.getElementById(id);
const els = {
    previewPane: $('previewPane'),
    previewWrap: $('previewWrap'),
    previewScroll: $('previewScroll'),
    previewArticle: $('previewArticle'),
    emptyState: $('emptyState'),
    loadingState: $('loadingState'),
    editorMount: $('editorMount'),
    docTitle: $('docTitle'),
    docStats: $('docStats'),
    fileInput: $('fileInput'),
    themeToggle: $('themeToggle'),
    skillLink: $('skillLink'),
    openBtn: $('openBtn'),
    viewBtns: Array.from(document.querySelectorAll('.view-btn')),
    findBtn: $('findBtn'),
    emptyOpenBtn: $('emptyOpenBtn'),
    emptyWriteBtn: $('emptyWriteBtn'),
    emptyExampleBtn: $('emptyExampleBtn'),
    urlForm: $('urlForm'),
    urlInput: $('urlInput'),
    optionsBtn: $('optionsBtn'),
    optionsPopover: $('optionsPopover'),
    optReadingWidth: $('optReadingWidth'),
    optReadingCustomWrap: $('optReadingCustomWrap'),
    optReadingCustom: $('optReadingCustom'),
    shareBtn: $('shareBtn'),
    shareMenu: $('shareMenu'),
    shortLinkAlert: $('shortLinkAlert'),
    fullscreenBtn: $('fullscreenBtn'),
    fullscreenEnterIcon: $('fullscreenEnterIcon'),
    fullscreenExitIcon: $('fullscreenExitIcon'),
    findBar: $('findBar'),
    findInput: $('findInput'),
    findCount: $('findCount'),
    findPrev: $('findPrev'),
    findNext: $('findNext'),
    findClose: $('findClose'),
    toast: $('toast'),
    toastMsg: $('toastMsg'),
    toastAction: $('toastAction'),
    saveState: $('saveState'),
    optHideDone: $('optHideDone'),
    blockHandle: $('blockHandle'),
    blockAddBtn: $('blockAddBtn'),
    blockGripBtn: $('blockGripBtn'),
    blockDropLine: $('blockDropLine'),
    formatBar: $('formatBar'),
    blockMenu: $('blockMenu'),
    commentsRail: $('commentsRail'),
    railResizer: $('railResizer'),
    commentsBtn: $('commentsBtn'),
    commentsCount: $('commentsCount'),
};

let markdown = '';
let frontmatter = ''; // Stripped before rendering, restored when saving.
let filename = 'Untitled';
let renderedMarkdown = ''; // Source of the current preview DOM; block ranges are only valid for it.
let savedMarkdown = ''; // Last loaded or saved-to-disk content, for the dirty indicator.
let threads = []; // Review comment threads, stored as a trailing `<!-- su-comments -->` block.
let commentsState = ''; // Serialized `threads`, refreshed by setThreads.
let savedComments = '';
let persistedText = ''; // documentText() last handed to the host for storage.
let persistedName = '';
let fileHandle = null; // File System Access handle (Chromium) for in-place saves.
let fileStamp = 0; // `lastModified` of the file at the last read or write.
let fileConflict = false; // The file changed on disk; autosave stops until an explicit save.
let renderTimer;
let toastTimer;
let viewMode = 'preview'; // 'preview' | 'edit'
const isEdit = () => viewMode === 'edit';
// True while a fragment / ?url= load is in flight; the head-set `su-loading`
// class drives the pre-JS paint, JS takes over from here.
let pendingHydration = document.documentElement.classList.contains('su-loading');
document.documentElement.classList.remove('su-loading');
updatePanes(); // hoisted; mirror the head-set loading class via JS-managed styles

const EXAMPLE_MD = [
    '# Welcome to MARS',
    '',
    'MARS is a **Markdown Annotation & Review System**. Keep your reviews in the sidebar, select text to leave a comment, then **Share** the review with a colleague. Their copy lands in their own sidebar, under **Shared with me**.',
    '',
    '> [!TIP]',
    '> Select any text in this document and click the speech bubble to start a review thread.',
    '',
    '## Features',
    '',
    '- Live preview with GitHub-flavored Markdown (tables, task lists, ~~strikethrough~~)',
    '- ==Highlighted== text via `==marks==`',
    '- Math with KaTeX: inline $E = mc^2$ and display blocks',
    '- Mermaid diagrams rendered to inline SVG',
    '- Interactive GeoJSON / TopoJSON maps and ASCII STL models',
    '- Review threads, folders, short links, and `.md` / `.zip` export',
    '',
    '- [x] Render Markdown',
    '- [ ] Profit',
    '',
    '## Math',
    '',
    'The Gaussian integral:',
    '',
    '$$\\int_{-\\infty}^{\\infty} e^{-x^2}\\,dx = \\sqrt{\\pi}$$',
    '',
    '## Code',
    '',
    '```js',
    'function greet(name) {',
    '    return `Hello, ${name}!`;',
    '}',
    'console.log(greet("world"));',
    '```',
    '',
    '## Diagram',
    '',
    '```mermaid',
    'flowchart LR',
    '    A[Open Markdown] --> B[Read the preview]',
    '    B --> C{Need to tweak?}',
    '    C -->|Yes| D[Edit source]',
    '    C -->|No| E[Share or Export]',
    '```',
    '',
    '## Map',
    '',
    '```geojson',
    '{"type":"Feature","properties":{"name":"Example region"},"geometry":{"type":"Polygon","coordinates":[[[-90,35],[-90,30],[-85,30],[-85,35],[-90,35]]]}}',
    '```',
    '',
    '## Topology',
    '',
    '```topojson',
    '{"type":"Topology","objects":{"route":{"type":"LineString","arcs":[0]}},"arcs":[[[0,0],[5,10],[10,0]]]}',
    '```',
    '',
    '## 3D model',
    '',
    '```stl',
    'solid corner',
    'facet normal 0 0 1',
    'outer loop',
    'vertex 0 0 0',
    'vertex 1 0 0',
    'vertex 0 1 0',
    'endloop',
    'endfacet',
    'facet normal 0 1 0',
    'outer loop',
    'vertex 0 0 0',
    'vertex 0 0 1',
    'vertex 1 0 0',
    'endloop',
    'endfacet',
    'endsolid corner',
    '```',
    '',
    '## Table',
    '',
    '| Feature                | Status |',
    '|------------------------|--------|',
    '| GFM (GitHub-flavoured) | yes    |',
    '| Syntax highlighting    | yes    |',
    '| Diagrams (Mermaid)     | yes    |',
    '| Math (KaTeX)           | yes    |',
    '',
    '## Quote',
    '',
    '> "The best Markdown is the one you can just read."',
    '',
    'See the [project on GitHub](https://github.com/secutils-dev/secutils) for source and discussions.',
    '',
].join('\n');

(() => {
    const root = document.documentElement;
    const setTheme = (t) => {
        root.setAttribute('data-theme', t);
        try { localStorage.setItem('su-tool-theme', t); } catch {}
    };
    els.themeToggle.addEventListener('click', () => {
        setTheme(root.getAttribute('data-theme') === 'dark' ? 'light' : 'dark');
    });
    if (els.skillLink) {
        const p = location.pathname;
        els.skillLink.href = (p === '/' || p === '') ? '/llms.txt' : p.replace(/\/$/, '') + '.md';
    }
    try {
        const saved = localStorage.getItem('su-tool-theme');
        if (saved) setTheme(saved);
        else if (window.matchMedia('(prefers-color-scheme: light)').matches) setTheme('light');
    } catch {}
})();

(() => {
    const dlg = $('privacyDialog');
    $('privacyOpen').addEventListener('click', () => dlg.showModal());
    $('privacyClose').addEventListener('click', () => dlg.close());
})();

(() => {
    const dlg = $('creditsDialog');
    $('creditsOpen').addEventListener('click', () => dlg.showModal());
    $('creditsClose').addEventListener('click', () => dlg.close());
})();

// Safari fallback for closedby="any" (Newly Baseline elsewhere; limited in Safari as of Q2 2026).
if (!('closedBy' in HTMLDialogElement.prototype)) {
    for (const dlg of document.querySelectorAll('dialog.su-dialog')) {
        dlg.addEventListener('click', (e) => {
            if (e.target !== dlg) return;
            const r = dlg.getBoundingClientRect();
            if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) dlg.close();
        });
    }
}

marked.use({
    gfm: true,
    breaks: false,
    hooks: {
        processAllTokens(tokens) {
            escapeUnmatchedInlineHtml(tokens);
            return tokens;
        },
    },
    extensions: [{
        name: 'highlight',
        level: 'inline',
        start(src) { return src.indexOf('=='); },
        tokenizer(src) {
            if (!src.startsWith('==') || this.lexer.state.inRawBlock) return;
            // Parse before HTML splits the delimiters across text nodes. Skip
            // escaped characters and complete code spans when finding the end.
            const delimiters = /\\[\s\S]|`+|==/g;
            delimiters.lastIndex = 2;
            let match;
            while ((match = delimiters.exec(src))) {
                if (match[0].startsWith('`')) {
                    const code = this.lexer.tokenizer.codespan(src.slice(match.index));
                    if (code) delimiters.lastIndex = match.index + code.raw.length;
                    continue;
                }
                if (match[0] !== '==') continue;
                const text = src.slice(2, match.index);
                const raw = src.slice(0, match.index + 2);
                // Consume an empty pair as literal text so its closing marker
                // cannot become the opener of a later highlight.
                if (!text.trim()) return { type: 'text', raw, text: raw };
                return {
                    type: 'highlight',
                    raw,
                    tokens: this.lexer.inlineTokens(text),
                };
            }
        },
        renderer(token) {
            return `<mark class="su-hl">${this.parser.parseInline(token.tokens)}</mark>`;
        },
    }],
});

const escHTML = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

const HTML_VOID_TAGS = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr']);
function escapeUnmatchedInlineHtml(value) {
    if (!value || typeof value !== 'object') return;
    if (Array.isArray(value)) {
        const open = [];
        for (const token of value) {
            if (token?.type !== 'html' || token.block) continue;
            const tag = token.text.match(/^<(\/?)([a-z][\w:-]*)\b[^>]*>$/i);
            if (!tag) continue;
            const name = tag[2].toLowerCase();
            if (HTML_VOID_TAGS.has(name)) continue;
            if (!tag[1]) {
                open.push({ name, token });
            } else {
                const index = open.findLastIndex((entry) => entry.name === name);
                if (index < 0) token.text = escHTML(token.text);
                else open.splice(index, 1);
            }
        }
        // Match within one inline token list, so a prose <Button> cannot
        // capture a later paragraph (or match an unrelated button there).
        for (const { token } of open) token.text = escHTML(token.text);
    }
    for (const child of Object.values(value)) escapeUnmatchedInlineHtml(child);
}

// Yield to the event loop between highlight passes so long documents
// don't block input. See dev/tools/guides/javascript.md -> "Long tasks (scheduler.yield)".
const yieldToMain = () => 'scheduler' in window && typeof scheduler.yield === 'function'
    ? scheduler.yield()
    : new Promise((r) => setTimeout(r, 0));

// ---- Mermaid (lazy-loaded ESM, pre-rendered to static SVG) ----
let _mermaidPromise;
function ensureMermaid() {
    if (!_mermaidPromise) {
        _mermaidPromise = import(MERMAID_URL).then((m) => {
            const mermaid = m.default;
            mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: 'default' });
            return mermaid;
        }).catch((err) => { _mermaidPromise = undefined; throw err; });
    }
    return _mermaidPromise;
}

function autoQuoteMermaidLabels(src) {
    return src.replace(/(\[)([^\[\]\n]*?[()][^\[\]\n]*?)(\])/g, (m, open, label, close) => {
        const t = label.trim();
        if (/^".*"$/.test(t) || /^'.*'$/.test(t)) return m;
        return `${open}"${t.replace(/"/g, '&quot;')}"${close}`;
    });
}

async function sanitizeMermaid(mermaid, src) {
    try {
        await mermaid.parse(src);
        return src;
    } catch (firstErr) {
        const fixed = autoQuoteMermaidLabels(src);
        if (fixed !== src) {
            try { await mermaid.parse(fixed); return fixed; } catch { /* fall through */ }
        }
        throw firstErr;
    }
}

const _mermaidCache = new Map();
let _mermaidId = 0;
async function renderMermaid(src) {
    if (_mermaidCache.has(src)) return _mermaidCache.get(src);
    let mermaid;
    try {
        mermaid = await ensureMermaid();
    } catch {
        const e = new Error('Mermaid failed to load');
        e.__suMermaidUnavailable = true;
        throw e;
    }
    const cleaned = await sanitizeMermaid(mermaid, src);
    const { svg } = await mermaid.render('su-mermaid-' + _mermaidId++, cleaned);
    _mermaidCache.set(src, svg);
    return svg;
}

async function replaceMermaidBlock(codeEl, forExport) {
    const pre = codeEl.parentElement;
    const src = codeEl.textContent;
    try {
        const svg = await renderMermaid(src);
        const fig = document.createElement('figure');
        fig.className = 'su-mermaid';
        fig.innerHTML = svg;
        if (!forExport) fig.insertAdjacentHTML('beforeend', '<div class="su-diagram-controls"><div class="su-diagram-actions"><button type="button" class="su-mermaid-fullscreen" title="Toggle diagram fullscreen" aria-label="Toggle diagram fullscreen"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/></svg></button></div></div>');
        pre.replaceWith(fig);
    } catch (err) {
        if (err && err.__suMermaidUnavailable) {
            try { hljs.highlightElement(codeEl); } catch {}
            pre.setAttribute('data-lang', 'mermaid');
            return;
        }
        const box = document.createElement('div');
        box.className = 'su-mermaid-error';
        const msg = (err && err.message) ? err.message : String(err);
        box.innerHTML = `<p class="su-mermaid-error-title">Unable to render Mermaid diagram</p>`
            + `<pre class="su-mermaid-error-msg">${escHTML(msg)}</pre>`
            + `<pre class="su-mermaid-error-src">${escHTML(src)}</pre>`;
        pre.replaceWith(box);
    }
}

// Maps use SVG; models use SVG or WebGL. Exported renderers work offline.
let topojsonPromise;
let stlParseCache;
let meshParseCache, modelWorkerSourcePromise;
const modelJobs = new Set();
async function modelWorkerSource() {
    modelWorkerSourcePromise ??= (async () => {
        const el = $('su-bundle-three-mf');
        if (!el.textContent) throw new Error('The 3MF importer bundle is not loaded.');
        let source = el.textContent.trim();
        if (el.dataset.suBundleEncoding === 'gzip-base64') {
            const bytes = Uint8Array.from(atob(source), (c) => c.charCodeAt(0));
            source = await new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'))).text();
        }
        const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
        try { return (await import(url)).workerSource; }
        finally { URL.revokeObjectURL(url); }
    })().catch((error) => { modelWorkerSourcePromise = undefined; throw error; });
    return modelWorkerSourcePromise;
}
async function runModelJob(input, signal) {
    const source = await modelWorkerSource();
    if (signal?.aborted) throw new DOMException('Import cancelled', 'AbortError');
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    const worker = new Worker(url);
    URL.revokeObjectURL(url);
    return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => { cleanup(); reject(new Error('Model processing exceeded 30 seconds.')); }, 30000);
        const cleanup = () => { clearTimeout(timeout); worker.terminate(); signal?.removeEventListener('abort', cancel); modelJobs.delete(cancel); };
        const cancel = () => { cleanup(); reject(new DOMException('Import cancelled', 'AbortError')); };
        modelJobs.add(cancel);
        signal?.addEventListener('abort', cancel, { once: true });
        worker.onmessage = ({ data }) => { cleanup(); if (data.error) reject(new Error(data.error)); else resolve(data); };
        worker.onerror = () => { cleanup(); reject(new Error('The 3MF worker failed.')); };
        worker.postMessage(input, input.buffer ? [input.buffer] : []);
    });
}
async function parseColoredMesh(source) {
    if (meshParseCache?.source === source.trim()) return meshParseCache.mesh;
    const result = await runModelJob({ source });
    Object.defineProperty(result.mesh, '__buffers', { value: { positions: result.positions, colors: result.colors } });
    meshParseCache = { source: source.trim(), mesh: result.mesh };
    return result.mesh;
}
window.addEventListener('pagehide', () => { for (const cancel of modelJobs) cancel(); });

// Fence metadata is tool-specific; geometry remains unchanged in the fence body.
function normalizeModelView(value = {}) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Model view must be a JSON object.');
    const defaults = { yaw: -0.6, pitch: 0.5, zoom: 1, panX: 0, panY: 0, mode: 'Solid' };
    const view = { ...defaults, ...value };
    if (Object.keys(value).some((key) => !(key in defaults))) throw new Error('Unknown model view setting.');
    for (const key of ['yaw', 'pitch', 'zoom', 'panX', 'panY']) {
        if (!Number.isFinite(view[key])) throw new Error(`Model view ${key} must be a finite number.`);
    }
    if (view.zoom < 0.1 || view.zoom > 20) throw new Error('Model view zoom must be from 0.1 to 20.');
    if (Math.abs(view.yaw) > 1000000 || Math.abs(view.pitch) > 1000000 || Math.abs(view.panX) > 1000000 || Math.abs(view.panY) > 1000000) throw new Error('Model view rotation or pan exceeds the supported range.');
    if (!['Solid', 'Wireframe', 'Surface angle'].includes(view.mode)) throw new Error('Unknown model view mode.');
    return view;
}
function parseModelView(info) {
    const metadata = info.trim().replace(/^(?:stl|su-mesh)\b/i, '').trim();
    if (!metadata) return normalizeModelView();
    if (!metadata.startsWith('view=')) throw new Error('Expected model fence metadata: view={...}');
    return normalizeModelView(JSON.parse(metadata.slice(5)));
}
async function parseStlMesh(src) {
    if (stlParseCache?.source === src) return stlParseCache.data;
    if (!/^\s*solid\b/i.test(src) || !/\bendsolid\b/i.test(src)) throw new Error('Expected an ASCII STL solid.');
    const number = '[+-]?(?:\\d+\\.?\\d*|\\.\\d+)(?:[eE][+-]?\\d+)?';
    const vertex = new RegExp(`vertex\\s+(${number})\\s+(${number})\\s+(${number})`, 'gi');
    const count = (src.match(/\bendfacet\b/gi) ?? []).length;
    const positions = new Float32Array(count * 9);
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    let index = 0, deadline = performance.now() + 20;
    for (const facet of src.matchAll(/facet\s+normal\b([\s\S]*?)endfacet/gi)) {
        let vertices = 0;
        for (const match of facet[1].matchAll(vertex)) {
            for (let axis = 0; axis < 3; axis++) {
                const value = Number(match[axis + 1]);
                if (!Number.isFinite(value) || !Number.isFinite(Math.fround(value))) throw new Error('STL vertices must be finite.');
                positions[index++] = value;
                min[axis] = Math.min(min[axis], value); max[axis] = Math.max(max[axis], value);
            }
            vertices++;
        }
        if (vertices !== 3) throw new Error('Each STL facet needs three vertices.');
        if (performance.now() >= deadline) { await yieldToMain(); deadline = performance.now() + 20; }
    }
    if (!index || index !== positions.length) throw new Error('The STL solid has no valid triangular facets.');
    let data;
    if (count > 2000) {
        const bytes = new Uint8Array(positions.buffer);
        const chunks = [];
        for (let i = 0; i < bytes.length; i += 24576) {
            chunks.push(btoa(String.fromCharCode(...bytes.subarray(i, i + 24576))));
            if (performance.now() >= deadline) { await yieldToMain(); deadline = performance.now() + 20; }
        }
        data = { positions: chunks.join(''), count, min, max };
    } else {
        data = [];
        for (let i = 0; i < positions.length; i += 9) data.push([Array.from(positions.subarray(i, i + 3)), Array.from(positions.subarray(i + 3, i + 6)), Array.from(positions.subarray(i + 6, i + 9))]);
    }
    stlParseCache = { source: src, data };
    return data;
}
async function replaceDiagramBlock(codeEl, kind, forExport = true) {
    const src = codeEl.textContent;
    const pre = codeEl.parentElement;
    try {
        let data;
        if (kind === 'stl') {
            data = await parseStlMesh(src);
        } else if (kind === 'su-mesh') {
            data = await parseColoredMesh(src);
        } else {
            data = JSON.parse(src);
            if (kind === 'topojson') {
                if (data.type !== 'Topology' || !data.objects || !Object.keys(data.objects).length) throw new Error('Expected a TopoJSON Topology with objects.');
                topojsonPromise ??= import(TOPOJSON_URL).catch((err) => { topojsonPromise = undefined; throw err; });
                const topojson = await topojsonPromise;
                data = { type: 'FeatureCollection', features: Object.values(data.objects).flatMap((object) => {
                    const converted = topojson.feature(data, object);
                    return converted.type === 'FeatureCollection' ? converted.features : [converted];
                }) };
            }
        }
        const fig = document.createElement('figure');
        fig.className = 'su-diagram';
        fig.dataset.kind = kind;
        if (kind === 'stl' || kind === 'su-mesh') {
            fig.dataset.view = JSON.stringify(parseModelView(codeEl.dataset.modelInfo ?? kind));
            if (codeEl.__suSourceRange) fig.__suSourceRange = codeEl.__suSourceRange;
        }
        const payload = document.createElement('script');
        payload.type = 'application/json';
        if (forExport || !data.positions) payload.textContent = JSON.stringify({ data, source: src }).replace(/</g, '\\u003c');
        else fig.__suPayload = { data, source: src };
        fig.appendChild(payload);
        // Validate and draw while detached; the serialized SVG is also the print view.
        if (!data.positions) initDiagrams(fig);
        pre.replaceWith(fig);
    } catch (err) {
        const box = document.createElement('div');
        box.className = 'su-mermaid-error';
        const title = document.createElement('p');
        title.className = 'su-mermaid-error-title';
        title.textContent = `Unable to render ${kind === 'stl' ? 'STL model' : kind === 'su-mesh' ? '3MF model' : `${kind} map`}`;
        const message = document.createElement('p');
        message.textContent = err?.message ?? String(err);
        box.append(title, message, pre.cloneNode(true));
        pre.replaceWith(box);
    }
}

// A single GPU buffer replaces thousands of SVG nodes for large meshes.
function createStlGpu(canvas, mesh) {
    const gl = canvas.getContext('webgl', { antialias: true, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL is required to display this model.');
    const decode = (encoded) => {
        const binary = atob(encoded), bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        return new Float32Array(bytes.buffer);
    };
    const positions = mesh.__buffers ? mesh.__buffers.positions.slice() : decode(mesh.positions);
    const center = mesh.min.map((value, axis) => (value + mesh.max[axis]) / 2);
    const span = Math.max(...mesh.max.map((value, axis) => value - mesh.min[axis]), 0.000001);
    for (let i = 0; i < positions.length; i++) positions[i] = (positions[i] - center[i % 3]) / span;
    const shader = (type, source) => {
        const result = gl.createShader(type); gl.shaderSource(result, source); gl.compileShader(result);
        if (!gl.getShaderParameter(result, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(result));
        return result;
    };
    const program = gl.createProgram();
    gl.attachShader(program, shader(gl.VERTEX_SHADER, `attribute vec3 position; attribute vec3 vertexColor; uniform vec4 view; uniform vec2 pan; varying vec3 point; varying vec3 surfaceColor;
        void main() { float cy=cos(view.x), sy=sin(view.x), cp=cos(view.y), sp=sin(view.y);
        vec3 p=vec3(position.x*cy+position.z*sy,position.y,-position.x*sy+position.z*cy);
        p=vec3(p.x,p.y*cp-p.z*sp,p.y*sp+p.z*cp); point=p; surfaceColor=vertexColor;
        gl_Position=vec4(p.x*view.z+pan.x,p.y*view.w+pan.y,-p.z*0.5,1.0); }`));
    const derivatives = gl.getExtension('OES_standard_derivatives');
    gl.attachShader(program, shader(gl.FRAGMENT_SHADER, `${derivatives ? '#extension GL_OES_standard_derivatives : enable\n' : ''}precision mediump float; varying vec3 point; varying vec3 surfaceColor; uniform float mode; uniform float colored;
        void main(){ float light=${derivatives ? 'abs(normalize(cross(dFdx(point),dFdy(point))).z)' : '0.7'};
        vec3 color=mix(vec3(0.12,0.42,0.72),vec3(0.42,0.72,0.94),light);
        if(colored>0.5) color=surfaceColor*(0.65+0.35*light);
        if(mode>0.5) color=mix(vec3(0.3,0.2,0.85),vec3(0.9,0.65,0.1),light);
        gl_FragColor=vec4(color,1.0); }`));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program));
    gl.useProgram(program);
    const upload = (values) => { const buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buffer); gl.bufferData(gl.ARRAY_BUFFER, values, gl.STATIC_DRAW); return buffer; };
    const solid = upload(positions);
    const colorLocation = gl.getAttribLocation(program, 'vertexColor');
    const colored = gl.getUniformLocation(program, 'colored');
    let colorBuffer;
    if (mesh.colors) {
        colorBuffer = upload(mesh.__buffers?.colors ?? decode(mesh.colors));
    }
    let wire;
    const location = gl.getAttribLocation(program, 'position'); gl.enableVertexAttribArray(location);
    const view = gl.getUniformLocation(program, 'view'), pan = gl.getUniformLocation(program, 'pan'), mode = gl.getUniformLocation(program, 'mode');
    gl.enable(gl.DEPTH_TEST);
    return (yaw, pitch, zoom, panX, panY, display) => {
        const width = Math.min(1600, Math.round((canvas.clientWidth || 800) * Math.min(devicePixelRatio || 1, 2)));
        const height = Math.round(width / ((canvas.clientWidth || 800) / (canvas.clientHeight || 400)));
        if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
        gl.viewport(0, 0, width, height); gl.clearColor(0.933, 0.953, 0.973, 1); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        if (display === 'Wireframe' && !wire) {
            const edges = new Float32Array(positions.length * 2);
            for (let i = 0, j = 0; i < positions.length; i += 9) for (const vertex of [0, 1, 1, 2, 2, 0]) for (let axis = 0; axis < 3; axis++) edges[j++] = positions[i + vertex * 3 + axis];
            wire = upload(edges);
        }
        gl.bindBuffer(gl.ARRAY_BUFFER, display === 'Wireframe' ? wire : solid); gl.vertexAttribPointer(location, 3, gl.FLOAT, false, 0, 0);
        if (colorBuffer && display !== 'Wireframe') {
            gl.enableVertexAttribArray(colorLocation); gl.bindBuffer(gl.ARRAY_BUFFER, colorBuffer); gl.vertexAttribPointer(colorLocation, 3, gl.FLOAT, false, 0, 0);
        } else { gl.disableVertexAttribArray(colorLocation); gl.vertexAttrib3f(colorLocation, 0.15, 0.45, 0.75); }
        gl.uniform1f(colored, colorBuffer ? 1 : 0);
        gl.uniform4f(view, yaw, pitch, 0.75 * zoom, 1.5 * zoom);
        gl.uniform2f(pan, panX / 400, -panY / 200); gl.uniform1f(mode, display === 'Surface angle' ? 1 : 0);
        gl.drawArrays(display === 'Wireframe' ? gl.LINES : gl.TRIANGLES, 0, positions.length / 3 * (display === 'Wireframe' ? 2 : 1));
    };
}

// Kept closure-free so the same renderer can be embedded in downloaded HTML.
function initDiagrams(root) {
    const figures = root.matches?.('figure.su-diagram') ? [root] : root.querySelectorAll('figure.su-diagram');
    const svgNS = 'http://www.w3.org/2000/svg';
    const makeSvg = (name, attrs) => {
        const el = document.createElementNS(svgNS, name);
        for (const [key, value] of Object.entries(attrs)) el.setAttribute(key, value);
        return el;
    };
    for (const fig of figures) {
        if (fig.__suDiagramReady) continue;
        fig.__suDiagramReady = true;
        const { data, source } = fig.__suPayload ?? JSON.parse(fig.querySelector('script[type="application/json"]').textContent);
        const colored = fig.dataset.kind === 'su-mesh';
        const stl = fig.dataset.kind === 'stl' || colored;
        const large = stl && !!data.positions;
        for (const el of fig.querySelectorAll(':scope > svg, :scope > canvas, :scope > figcaption, :scope > .su-diagram-controls')) el.remove();
        const svg = large ? document.createElement('canvas') : makeSvg('svg', { viewBox: '0 0 800 400' });
        svg.classList.add('su-diagram-view'); svg.tabIndex = 0; svg.setAttribute('role', 'img');
        svg.setAttribute('aria-label', stl ? `Interactive ${colored ? '3MF' : 'STL'} model. Arrow keys rotate. Shift and arrow keys pan. Plus and minus zoom.` : 'Interactive map. Arrow keys pan. Plus and minus zoom.');
        let gpu;
        try { gpu = large ? createStlGpu(svg, data) : null; }
        catch (error) {
            svg.getContext('webgl')?.getExtension('WEBGL_lose_context')?.loseContext();
            const message = document.createElement('p'); message.textContent = error.message;
            fig.appendChild(message); continue;
        }
        const caption = document.createElement('figcaption');
        if (!stl) {
            const attribution = document.createElement('a');
            attribution.href = 'https://www.openstreetmap.org/copyright'; attribution.target = '_blank'; attribution.rel = 'noopener';
            attribution.textContent = '© OpenStreetMap contributors';
            caption.textContent = 'Drag to pan; use + / − to zoom. ';
            caption.appendChild(attribution);
        }
        const controls = document.createElement('div');
        controls.className = 'su-diagram-controls';
        controls.setAttribute('role', 'group');
        controls.setAttribute('aria-label', stl ? 'Model controls' : 'Map controls');
        const group = (name) => { const el = document.createElement('div'); el.className = `su-diagram-${name}`; controls.appendChild(el); return el; };
        const actions = group('actions'), navigation = group('navigation'), modesGroup = group('modes'), rotation = group('rotation');
        const mercator = (point) => {
            const latitude = Math.max(-85.05112878, Math.min(85.05112878, point[1])) * Math.PI / 180;
            return [(point[0] + 180) / 360, (1 - Math.log(Math.tan(Math.PI / 4 + latitude / 2)) / Math.PI) / 2];
        };
        const initial = { yaw: -0.6, pitch: 0.5, zoom: 1, panX: 0, panY: 0, mode: 'Solid', ...(stl && fig.dataset.view ? JSON.parse(fig.dataset.view) : {}) };
        let { zoom, panX, panY, yaw, pitch, mode } = initial;
        const currentView = () => ({ yaw, pitch, zoom, panX, panY, mode });
        fig.__suGetView = currentView;
        const features = [];
        const allPoints = [];
        const collectGeometry = (geometry, properties = {}) => {
            if (!geometry) return;
            if (geometry.type === 'FeatureCollection') {
                if (!Array.isArray(geometry.features)) throw new Error('Expected a GeoJSON features array.');
                for (const feature of geometry.features) collectGeometry(feature);
                return;
            }
            if (geometry.type === 'Feature') { collectGeometry(geometry.geometry, geometry.properties ?? {}); return; }
            if (geometry.type === 'GeometryCollection') { for (const child of geometry.geometries) collectGeometry(child, properties); return; }
            const depths = { Point: 0, MultiPoint: 1, LineString: 1, MultiLineString: 2, Polygon: 2, MultiPolygon: 3 };
            if (!(geometry.type in depths)) throw new Error('Unsupported GeoJSON geometry type.');
            const visit = (coords, depth) => {
                if (!Array.isArray(coords)) throw new Error('Expected GeoJSON coordinates.');
                if (depth) { for (const child of coords) visit(child, depth - 1); return; }
                if (coords.length < 2 || !Number.isFinite(coords[0]) || !Number.isFinite(coords[1]) || Math.abs(coords[0]) > 180 || Math.abs(coords[1]) > 90) throw new Error('Expected longitude from −180 to 180 and latitude from −90 to 90.');
                allPoints.push(mercator(coords));
            };
            visit(geometry.coordinates, depths[geometry.type]);
            features.push({ ...geometry, properties });
        };
        if (large) {
            allPoints.push(data.min, data.max);
        } else if (stl) {
            for (const triangle of data) allPoints.push(...triangle);
        } else collectGeometry(data);
        if (!allPoints.length) throw new Error('The map has no coordinates to display.');
        const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
        for (const point of allPoints) for (let i = 0; i < (stl ? 3 : 2); i++) { min[i] = Math.min(min[i], point[i]); max[i] = Math.max(max[i], point[i]); }
        const center = min.map((n, i) => (n + max[i]) / 2);
        const span = Math.max(max[0] - min[0], max[1] - min[1], stl ? max[2] - min[2] : 0, stl ? 0.000001 : 0.0003);
        const baseScale = 300 / span;
        const project = (point) => {
            let x = point[0] - center[0], y = point[1] - center[1], z = stl ? point[2] - center[2] : 0;
            if (stl) {
                [x, z] = [x * Math.cos(yaw) + z * Math.sin(yaw), -x * Math.sin(yaw) + z * Math.cos(yaw)];
                [y, z] = [y * Math.cos(pitch) - z * Math.sin(pitch), y * Math.sin(pitch) + z * Math.cos(pitch)];
                y = -y;
            }
            return [400 + panX + x * baseScale * zoom, 200 + panY + y * baseScale * zoom, z * baseScale * zoom];
        };
        const draw = () => {
            if (stl) fig.dataset.currentView = JSON.stringify(currentView());
            if (gpu) { gpu(yaw, pitch, zoom, panX, panY, mode); return; }
            svg.replaceChildren();
            if (stl) {
                const triangles = data.map((triangle) => triangle.map(project));
                triangles.sort((a, b) => a.reduce((sum, p) => sum + p[2], 0) - b.reduce((sum, p) => sum + p[2], 0));
                for (const triangle of triangles) {
                    const [a, b, c] = triangle;
                    const u = b.map((n, i) => n - a[i]), v = c.map((n, i) => n - a[i]);
                    const normal = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    const light = Math.abs(normal[2]) / (Math.hypot(...normal) || 1);
                    svg.appendChild(makeSvg('polygon', { points: triangle.map((p) => `${p[0]},${p[1]}`).join(' '), fill: mode === 'Wireframe' ? 'none' : `hsl(${mode === 'Surface angle' ? 240 - light * 200 : 210} 65% ${45 + light * 20}%)`, stroke: mode === 'Wireframe' ? '#2674bf' : 'none', 'stroke-width': '0.6' }));
                }
            } else {
                const worldSize = baseScale * zoom;
                const level = Math.max(0, Math.min(19, Math.floor(Math.log2(worldSize / 256))));
                const count = 2 ** level, tileSize = worldSize / count;
                const origin = project([0, 0]);
                const startX = Math.floor(-origin[0] / tileSize), startY = Math.floor(-origin[1] / tileSize);
                for (let x = startX; x <= startX + Math.ceil(800 / tileSize); x++) {
                    for (let y = Math.max(0, startY); y <= Math.min(count - 1, startY + Math.ceil(400 / tileSize)); y++) {
                        const tile = makeSvg('image', { x: origin[0] + x * tileSize, y: origin[1] + y * tileSize, width: tileSize + 0.5, height: tileSize + 0.5, href: `https://tile.openstreetmap.org/${level}/${((x % count) + count) % count}/${y}.png` });
                        svg.appendChild(tile);
                    }
                }
                const line = (coords) => coords.map((p, i) => { const q = project(mercator(p)); return `${i ? 'L' : 'M'}${q[0]},${q[1]}`; }).join('');
                for (const feature of features) {
                    const { type, coordinates, properties } = feature;
                    const group = makeSvg('g', {});
                    const title = makeSvg('title', {});
                    title.textContent = JSON.stringify(properties);
                    group.appendChild(title);
                    const color = (value, fallback) => typeof value === 'string' && /^#[\da-f]{3}(?:[\da-f]{3})?$/i.test(value) ? value : fallback;
                    if (type === 'Point' || type === 'MultiPoint') {
                        for (const point of type === 'Point' ? [coordinates] : coordinates) {
                            const p = project(mercator(point));
                            group.appendChild(makeSvg('circle', { cx: p[0], cy: p[1], r: 5, fill: color(properties['marker-color'], '#2674bf'), stroke: '#fff' }));
                        }
                    } else {
                        const polygon = type.includes('Polygon');
                        const parts = type === 'LineString' ? [coordinates] : type === 'MultiPolygon' ? coordinates.flat() : coordinates;
                        group.appendChild(makeSvg('path', { d: parts.map((part) => `${line(part)}${polygon ? 'Z' : ''}`).join(''), fill: polygon ? color(properties.fill, '#8957e5') : 'none', 'fill-opacity': '0.3', 'fill-rule': 'evenodd', stroke: color(properties.stroke, '#2674bf'), 'stroke-width': '2' }));
                    }
                    svg.appendChild(group);
                }
            }
        };
        const button = (label, action, parent = navigation, icon) => {
            const btn = document.createElement('button');
            btn.type = 'button';
            btn.textContent = label;
            btn.setAttribute('aria-label', label);
            btn.title = label;
            if (icon) btn.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${icon}</svg>`;
            btn.addEventListener('click', async () => { await action(); draw(); });
            parent.appendChild(btn);
            return btn;
        };
        const scale = (factor) => { zoom = Math.max(0.1, Math.min(20, zoom * factor)); };
        button('Zoom in', () => scale(1.25), navigation, '<path d="M12 5v14M5 12h14"/>');
        button('Zoom out', () => scale(0.8), navigation, '<path d="M5 12h14"/>');
        button('Reset view', () => { ({ zoom, panX, panY, yaw, pitch } = initial); }, navigation, '<path d="M4 10a8 8 0 1 1 1 8M4 4v6h6"/>');
        const announce = (text) => {
            fig.querySelector('.su-diagram-status')?.remove();
            const status = document.createElement('span');
            status.className = 'su-diagram-status'; status.setAttribute('role', 'status'); status.textContent = text;
            fig.appendChild(status); setTimeout(() => status.remove(), 2200);
        };
        if (stl && fig.__suSourceRange) {
            button('Save current view to source', () => {
                fig.dispatchEvent(new CustomEvent('su-save-model-view', { bubbles: true, detail: currentView() }));
            }, actions, '<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2Z"/><path d="M7 3v6h10V4M7 21v-8h10v8"/>');
        }
        button('Copy source', async () => {
            try { await navigator.clipboard.writeText(source); announce('Copied to clipboard'); }
            catch { announce('Failed to copy'); }
        }, actions, '<rect x="8" y="8" width="12" height="13" rx="2"/><path d="M5 16H4V3h12v2"/>');
        button('Toggle viewer fullscreen', async () => {
            try { if (document.fullscreenElement === fig) await document.exitFullscreen(); else await fig.requestFullscreen(); }
            catch { announce('Fullscreen is unavailable'); }
        }, actions, '<path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/>');
        if (stl) {
            const modes = [];
            for (const label of ['Solid', 'Wireframe', 'Surface angle']) {
                const btn = button(label, () => { mode = label; for (const item of modes) item.setAttribute('aria-pressed', String(item.textContent === mode)); }, modesGroup);
                btn.setAttribute('aria-pressed', String(label === mode));
                modes.push(btn);
            }
        }
        const move = (dx, dy) => { if (stl) { yaw += dx / 100; pitch += dy / 100; } else { panX += dx; panY += dy; } draw(); };
        const pan = (dx, dy) => { panX += dx; panY += dy; draw(); };
        if (stl) {
            const panControls = document.createElement('div');
            panControls.className = 'su-diagram-pan';
            panControls.setAttribute('role', 'group');
            panControls.setAttribute('aria-label', 'Pan model');
            navigation.prepend(panControls);
            button('Pan up', () => pan(0, -20), panControls, '<path d="m5 14 7-7 7 7"/>');
            button('Pan left', () => pan(-20, 0), panControls, '<path d="m14 5-7 7 7 7"/>');
            button('Pan right', () => pan(20, 0), panControls, '<path d="m10 5 7 7-7 7"/>');
            button('Pan down', () => pan(0, 20), panControls, '<path d="m5 10 7 7 7-7"/>');
            button('Rotate left', () => move(-20, 0), rotation, '<path d="m14 5-7 7 7 7"/>');
            button('Rotate right', () => move(20, 0), rotation, '<path d="m10 5 7 7-7 7"/>');
            button('Rotate up', () => move(0, -20), rotation, '<path d="m5 14 7-7 7 7"/>');
            button('Rotate down', () => move(0, 20), rotation, '<path d="m5 10 7 7 7-7"/>');
            button('Reset rotation', () => { yaw = initial.yaw; pitch = initial.pitch; }, rotation, '<path d="M4 10a8 8 0 1 1 1 8M4 4v6h6"/>');
        }
        let pointer;
        svg.addEventListener('pointerdown', (event) => { if (event.button !== 0) return; pointer = [event.clientX, event.clientY]; svg.setPointerCapture(event.pointerId); });
        svg.addEventListener('pointermove', (event) => {
            if (!pointer) return;
            const bounds = svg.getBoundingClientRect();
            const ratio = Math.max(800 / bounds.width, 400 / bounds.height);
            const change = stl && event.shiftKey ? pan : move;
            change((event.clientX - pointer[0]) * ratio, (event.clientY - pointer[1]) * ratio);
            pointer = [event.clientX, event.clientY];
        });
        svg.addEventListener('lostpointercapture', () => { pointer = null; });
        svg.addEventListener('keydown', (event) => {
            const arrows = { ArrowLeft: [-20, 0], ArrowRight: [20, 0], ArrowUp: [0, -20], ArrowDown: [0, 20] };
            if (arrows[event.key]) { event.preventDefault(); const change = stl && event.shiftKey ? pan : move; change(...arrows[event.key]); }
            else if (['+', '=', '-'].includes(event.key)) { event.preventDefault(); scale(event.key === '-' ? 0.8 : 1.25); draw(); }
        });
        fig.prepend(svg);
        const details = fig.querySelector('details');
        if (!stl) fig.insertBefore(caption, details);
        fig.insertBefore(controls, details);
        draw();
        if (large && !fig.isConnected) {
            fig.querySelector('.su-stl-snapshot')?.remove();
            const snapshot = document.createElement('img'); snapshot.className = 'su-stl-snapshot'; snapshot.alt = 'STL model';
            snapshot.src = svg.toDataURL('image/png'); fig.appendChild(snapshot);
        }
        if (large) {
            let frame, resize;
            const printSnapshot = () => {
                draw();
                let snapshot = fig.querySelector('.su-stl-snapshot');
                if (!snapshot) { snapshot = document.createElement('img'); snapshot.className = 'su-stl-snapshot'; snapshot.alt = colored ? '3MF model' : 'STL model'; fig.appendChild(snapshot); }
                snapshot.src = svg.toDataURL('image/png');
            };
            window.addEventListener('beforeprint', printSnapshot);
            if (fig.isConnected) {
                resize = new ResizeObserver(() => { cancelAnimationFrame(frame); frame = requestAnimationFrame(draw); });
                resize.observe(svg);
            }
            fig.__suDispose = () => {
                window.removeEventListener('beforeprint', printSnapshot);
                cancelAnimationFrame(frame); resize?.disconnect();
                svg.getContext('webgl')?.getExtension('WEBGL_lose_context')?.loseContext();
            };
        }
    }
}

// ---- KaTeX (lazy-loaded ESM; delimiter scanner ported from md-preview) ----
let _katexPromise;
function injectKatexCss() {
    if (document.getElementById('su-katex-css')) return;
    const link = document.createElement('link');
    link.id = 'su-katex-css';
    link.rel = 'stylesheet';
    link.href = KATEX_CSS_URL;
    document.head.appendChild(link);
}
function ensureKatex() {
    if (!_katexPromise) {
        _katexPromise = import(KATEX_JS_URL).then((m) => {
            injectKatexCss();
            return m.default;
        }).catch((err) => { _katexPromise = undefined; throw err; });
    }
    return _katexPromise;
}

// True when the document plausibly contains math; gates the KaTeX download.
function needsMath(md) {
    return /\$\$[\s\S]+?\$\$/.test(md)
        || /\\\([\s\S]+?\\\)/.test(md)
        || /\\\[[\s\S]+?\\\]/.test(md)
        || /(^|[^\\])\$[^\s$][\s\S]*?[^\s\\]\$/m.test(md);
}

function hasClassInTree(el, cls) {
    while (el && el !== document.body) {
        if (el.classList && el.classList.contains(cls)) return true;
        el = el.parentElement;
    }
    return false;
}
function shouldSkipTextNode(node) {
    const el = node.parentElement;
    if (!el) return true;
    const tag = el.tagName;
    if (tag === 'CODE' || tag === 'PRE' || tag === 'KBD' || tag === 'SAMP'
        || tag === 'SCRIPT' || tag === 'STYLE' || tag === 'TEXTAREA' || tag === 'A') return true;
    return hasClassInTree(el, 'katex') || hasClassInTree(el, 'su-hl')
        || hasClassInTree(el, 'su-mermaid') || hasClassInTree(el, 'su-mermaid-error') || hasClassInTree(el, 'su-diagram');
}
function isEscaped(text, index) {
    let count = 0;
    for (let i = index - 1; i >= 0 && text[i] === '\\'; i--) count++;
    return count % 2 === 1;
}
function findClosing(text, start, delim) {
    for (let i = start; i < text.length; i++) {
        if (delim === '$') {
            if (text[i] === '$' && !isEscaped(text, i) && text[i - 1] && !/\s/.test(text[i - 1])) return i;
        } else if (text.substr(i, delim.length) === delim && !isEscaped(text, i)) {
            return i;
        }
    }
    return -1;
}
function nextDelimiter(text, index) {
    if (text.substr(index, 2) === '$$' && !isEscaped(text, index)) return { open: '$$', close: '$$', display: true };
    if (text.substr(index, 2) === '\\[' && !isEscaped(text, index)) return { open: '\\[', close: '\\]', display: true };
    if (text.substr(index, 2) === '\\(' && !isEscaped(text, index)) return { open: '\\(', close: '\\)', display: false };
    if (text[index] === '$' && !isEscaped(text, index) && text[index + 1] && text[index + 1] !== '$' && !/\s/.test(text[index + 1])) {
        return { open: '$', close: '$', display: false };
    }
    return null;
}
function mathParts(text) {
    const parts = [];
    let i = 0, last = 0;
    while (i < text.length) {
        const delim = nextDelimiter(text, i);
        if (!delim) { i++; continue; }
        const bodyStart = i + delim.open.length;
        const closeAt = findClosing(text, bodyStart, delim.close);
        if (closeAt < 0) { i += delim.open.length; continue; }
        if (i > last) parts.push({ text: text.slice(last, i) });
        parts.push({ math: text.slice(bodyStart, closeAt), display: delim.display });
        i = closeAt + delim.close.length;
        last = i;
    }
    if (!parts.length) return null;
    if (last < text.length) parts.push({ text: text.slice(last) });
    return parts;
}
async function enhanceMath(root) {
    let katex;
    try { katex = await ensureKatex(); } catch { return; }
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => {
            if (shouldSkipTextNode(node)) return NodeFilter.FILTER_REJECT;
            return /(\$|\\\(|\\\[)/.test(node.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
        },
    });
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const node of nodes) {
        const parts = mathParts(node.nodeValue);
        if (!parts) continue;
        const frag = document.createDocumentFragment();
        for (const part of parts) {
            if (part.text !== undefined) { frag.appendChild(document.createTextNode(part.text)); continue; }
            const span = document.createElement('span');
            try {
                katex.render(part.math, span, { displayMode: part.display, throwOnError: false, strict: 'warn', trust: false });
            } catch (err) {
                span.className = 'su-math-error';
                span.textContent = part.display ? `$$${part.math}$$` : `$${part.math}$`;
                span.title = err && err.message ? err.message : 'KaTeX render error';
            }
            frag.appendChild(span);
        }
        node.parentNode.replaceChild(frag, node);
    }
}

// ---- GitHub alerts ([!NOTE] etc.) ----
const ALERT_LABELS = { note: 'Note', tip: 'Tip', important: 'Important', warning: 'Warning', caution: 'Caution' };
function enhanceAlerts(root) {
    for (const quote of root.querySelectorAll('blockquote')) {
        const first = quote.firstElementChild || quote;
        if (!first || !first.firstChild || first.firstChild.nodeType !== Node.TEXT_NODE) continue;
        const match = first.firstChild.nodeValue.match(/^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/i);
        if (!match) continue;
        const type = match[1].toLowerCase();
        quote.classList.add('markdown-alert', 'markdown-alert-' + type);
        first.firstChild.nodeValue = first.firstChild.nodeValue.slice(match[0].length);
        // Drop a now-empty leading <br> and whitespace.
        while (first.firstChild && ((first.firstChild.nodeType === Node.TEXT_NODE && !first.firstChild.nodeValue.trim()) || first.firstChild.nodeName === 'BR')) {
            first.removeChild(first.firstChild);
        }
        if (first !== quote && !first.children.length && !first.textContent.trim()) quote.removeChild(first);
        const title = document.createElement('p');
        title.className = 'markdown-alert-title';
        title.textContent = ALERT_LABELS[type];
        quote.insertBefore(title, quote.firstChild);
    }
}

// ---- Wide-table horizontal scroll wrap ----
function enhanceWideTables(root) {
    for (const table of root.querySelectorAll('table')) {
        if (table.parentElement && table.parentElement.classList.contains('mdp-table-wrap')) continue;
        const firstRow = table.querySelector('tr');
        if (!firstRow || firstRow.children.length < 4) continue;
        const wrap = document.createElement('div');
        wrap.className = 'mdp-table-wrap';
        table.parentNode.insertBefore(wrap, table);
        wrap.appendChild(table);
    }
}

// ---- Heading anchors (slug ids + hover anchor link) ----
function slugify(s) {
    return String(s || '').toLowerCase().trim().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '');
}
function enhanceHeadingAnchors(root) {
    const seen = new Map();
    for (const h of root.querySelectorAll('h1, h2, h3, h4, h5, h6')) {
        if (h.querySelector('.su-anchor')) continue;
        const base = slugify(h.textContent) || 'section';
        const n = seen.get(base) || 0;
        seen.set(base, n + 1);
        if (!h.id) h.id = n ? `${base}-${n}` : base;
        const a = document.createElement('a');
        a.className = 'su-anchor';
        a.href = `#${h.id}`;
        a.setAttribute('aria-label', `Link to "${h.textContent.trim()}"`);
        a.textContent = '#';
        h.appendChild(a);
    }
}

// ---- Source-mapped preview blocks ----
// The preview wraps every top-level block in an element that carries its exact
// source range, so edits splice the Markdown instead of re-serializing it.
const LIST_ITEM_RE = /^([ \t]*(?:[-*+]|\d{1,9}[.)])[ \t]+)(\[[ xX]\])?/;
function renderSourceBlocks(md, renderer) {
    const tokens = marked.lexer(md);
    escapeUnmatchedInlineHtml(tokens);
    const options = { ...marked.defaults, renderer };
    const defaultListitem = renderer.listitem;
    let cursor = 0, itemCursor = 0;
    renderer.listitem = function (item) {
        const firstLine = item.raw.split('\n', 1)[0];
        const pos = md.indexOf(firstLine, itemCursor);
        if (pos < 0) return defaultListitem.call(this, item);
        itemCursor = pos + firstLine.length;
        const exact = md.startsWith(item.raw, pos);
        const lineStart = md.lastIndexOf('\n', pos - 1) + 1;
        const marker = firstLine.match(LIST_ITEM_RE);
        const attrs = [];
        if (item.task && marker?.[2]) attrs.push(`data-check="${pos + marker[1].length}"`);
        // Whole-item ranges need an exact match and a whitespace-only indent. Items
        // in blockquotes or with dedented nested continuations stay checkbox-only.
        if (exact && !md.slice(lineStart, pos).trim()) attrs.push(`data-start="${lineStart}" data-end="${pos + item.raw.length}"`);
        const html = defaultListitem.call(this, item);
        if (exact) itemCursor = Math.max(itemCursor, pos + item.raw.length);
        return attrs.length ? html.replace(/^<li/, `<li ${attrs.join(' ')}`) : html;
    };
    let html = '';
    for (const token of tokens) {
        if (token.type === 'space') continue;
        // Link definitions are dropped from the token list, so locate each block
        // instead of summing raw lengths. Unlocatable blocks render read-only.
        const start = md.indexOf(token.raw, cursor);
        itemCursor = start < 0 ? cursor : start;
        const out = marked.parser(Object.assign([token], { links: tokens.links }), options);
        if (start < 0) { html += out; continue; }
        cursor = start + token.raw.length;
        const depth = token.type === 'heading' ? ` data-depth="${token.depth}"` : '';
        html += `<div class="su-block" data-start="${start}" data-end="${cursor}"${depth}>${out}</div>`;
    }
    return html;
}

// `comment` escapes raw HTML and skips heading ids, which would collide with the document's.
async function render(md, forExport = true, comment = false) {
    const el = document.createElement('div');
    // Direct model imports are one fence. Avoid escaping and reparsing tens of
    // megabytes through marked and the HTML parser just to recover the source.
    const standaloneModel = md.length > 1000000 && md.match(/^(`{3,})(stl|su-mesh)([^\r\n]*)\r?\n([\s\S]*)\r?\n\1\s*$/);
    if (standaloneModel) {
        const pre = document.createElement('pre'), code = document.createElement('code');
        code.className = `language-${standaloneModel[2]}`; code.textContent = standaloneModel[4];
        code.dataset.modelInfo = `${standaloneModel[2]}${standaloneModel[3]}`;
        if (!forExport) code.__suSourceRange = { start: standaloneModel[1].length, end: md.indexOf('\n'), fence: standaloneModel[1] };
        pre.appendChild(code); el.appendChild(pre);
    } else {
        let sourceCursor = 0;
        const renderer = new marked.Renderer();
        if (comment) renderer.html = ({ text }) => escHTML(text);
        const defaultCode = renderer.code;
        renderer.code = function (token) {
            if (!/^(?:stl|su-mesh)(?:\s|$)/i.test(token.lang ?? '')) return defaultCode.call(this, token);
            const start = md.indexOf(token.raw, sourceCursor);
            const opening = token.raw.match(/^ {0,3}(`{3,}|~{3,})([^\r\n]*)\r?\n/);
            let range = '';
            if (start >= 0 && opening) {
                sourceCursor = start + token.raw.length;
                const infoStart = start + opening[0].indexOf(opening[1]) + opening[1].length;
                if (!forExport) range = ` data-model-range="${escHTML(JSON.stringify({ start: infoStart, end: start + opening[0].length - (opening[0].endsWith('\r\n') ? 2 : 1), fence: opening[1] })).replace(/"/g, '&quot;')}"`;
            }
            return `<pre><code class="language-${token.lang.split(/\s/)[0].toLowerCase()}" data-model-info="${escHTML(token.lang).replace(/"/g, '&quot;')}"${range}>${escHTML(token.text)}</code></pre>`;
        };
        // marked normalizes CRLF, so source offsets only hold for LF text (see loadDocument).
        el.innerHTML = forExport || md.length > 10000000 || md.includes('\r')
            ? marked.parse(md, { renderer })
            : renderSourceBlocks(md, renderer);
        for (const code of el.querySelectorAll('code[data-model-range]')) {
            code.__suSourceRange = JSON.parse(code.dataset.modelRange); delete code.dataset.modelRange;
        }
    }
    let deadline = performance.now() + 50;
    for (const block of el.querySelectorAll('pre code')) {
        const langCls = [...block.classList].find((c) => c.startsWith('language-'));
        if (langCls === 'language-mermaid') {
            await replaceMermaidBlock(block, forExport);
            continue;
        }
        const diagramKind = langCls?.slice('language-'.length).toLowerCase();
        if (['geojson', 'topojson', 'stl', 'su-mesh'].includes(diagramKind)) {
            await replaceDiagramBlock(block, diagramKind, forExport);
            continue;
        }
        try { hljs.highlightElement(block); } catch {}
        if (langCls) block.parentElement.setAttribute('data-lang', langCls.replace('language-', ''));
        if (performance.now() >= deadline) {
            await yieldToMain();
            deadline = performance.now() + 50;
        }
    }
    enhanceAlerts(el);
    if (needsMath(md)) { try { await enhanceMath(el); } catch {} }
    enhanceWideTables(el);
    if (!comment) enhanceHeadingAnchors(el);
    if (!forExport) return el;
    initDiagrams(el);
    const html = el.innerHTML;
    for (const fig of el.querySelectorAll('.su-diagram')) fig.__suDispose?.();
    return html;
}

els.previewArticle.addEventListener('su-save-model-view', (event) => {
    const fig = event.target.closest('figure.su-diagram');
    const range = fig?.__suSourceRange;
    if (!range || fig.__suDocument !== markdown) { toast('The document changed. Save the view after the preview updates.'); return; }
    try {
        const view = normalizeModelView(event.detail);
        const info = `${fig.dataset.kind} view=${JSON.stringify(view)}`;
        setMarkdown(`${markdown.slice(0, range.start)}${info}${markdown.slice(range.end)}`);
        toast('Current view saved to source');
    } catch (error) { toast(error.message); }
});

// Splits off what the preview doesn't render: YAML frontmatter and the comments block.
function parseSource(md) {
    const m = md.match(FRONTMATTER_RE);
    frontmatter = m?.[0] ?? '';
    const t = m?.[1].match(FRONTMATTER_TITLE_RE);
    if (t) filename = t[1];
    const body = m ? md.slice(m[0].length) : md;
    const c = body.match(COMMENTS_RE);
    let parsed = null;
    try { parsed = c && JSON.parse(c[1]); } catch {}
    // A malformed block stays in the body, where Source view can repair it.
    setThreads(Array.isArray(parsed) ? parsed.filter(isThread) : []);
    return Array.isArray(parsed) ? body.slice(0, c.index) : body;
}

function setDocTitle() {
    if (document.activeElement === els.docTitle) return;
    const named = !!markdown.trim() || !!host.currentId?.();
    els.docTitle.textContent = named ? filename : '';
    els.docTitle.contentEditable = named ? 'plaintext-only' : 'false';
}
els.docTitle.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); els.docTitle.blur(); }
    else if (e.key === 'Escape') { e.stopPropagation(); els.docTitle.textContent = filename; els.docTitle.blur(); }
});
els.docTitle.addEventListener('blur', () => {
    const name = els.docTitle.textContent.trim().replace(MD_FILE_EXT, '');
    if (name && name !== filename) { filename = name; persistSoon(); }
    setDocTitle();
});

function updatePanes() {
    const hasContent = !!markdown.trim();
    els.loadingState.style.display = (pendingHydration && !hasContent) ? 'flex' : 'none';
    els.emptyState.style.display = (!pendingHydration && !hasContent) ? 'flex' : 'none';
    if (hasContent) els.shortLinkAlert.hidden = true;
    els.previewArticle.style.display = hasContent ? '' : 'none';
}

let previewGeneration = 0;
async function updatePreview() {
    const generation = ++previewGeneration;
    pendingHydration = false;
    if (!markdown.trim()) {
        for (const fig of els.previewArticle.querySelectorAll('.su-diagram')) fig.__suDispose?.();
        els.previewArticle.innerHTML = '';
        renderedMarkdown = markdown;
        blockEditor = null;
        updatePanes();
        renderedHook();
        anchorThreads();
        renderRail();
        els.docStats.textContent = 'empty';
        setDocTitle();
        setExportEnabled(false);
        persistSoon();
        return;
    }
    const source = markdown;
    const content = await render(source, false);
    if (generation !== previewGeneration) return;
    if (source.length > 1000000) await yieldToMain();
    if (generation !== previewGeneration) return;
    for (const fig of els.previewArticle.querySelectorAll('.su-diagram')) fig.__suDispose?.();
    for (const fig of content.querySelectorAll('figure.su-diagram')) fig.__suDocument = source;
    const hovered = hoverTarget && `${hoverTarget.matches('li') ? 'li' : '.su-block'}[data-start="${hoverTarget.dataset.start}"]`;
    els.previewArticle.replaceChildren(...content.childNodes);
    renderedMarkdown = source;
    if (blockEditor && !blockEditor.wrap.isConnected) blockEditor = null;
    initDiagrams(els.previewArticle);
    decorateBlocks();
    restoreScrollAnchor();
    updatePanes();
    renderedHook();
    anchorThreads();
    renderRail();
    restoreFormatSelection();
    if (queuedFormat) { const kind = queuedFormat; queuedFormat = null; applyFormat(kind); }
    const target = hovered && els.previewArticle.querySelector(hovered);
    if (target) showHandle(target); else hideHandle();
    const wordCount = source.length > 1000000 ? null : source.trim().split(/\s+/).length;
    els.docStats.textContent = wordCount === null ? `${(source.length / 1000000).toFixed(1)} MB` : `${wordCount} word${wordCount === 1 ? '' : 's'}${progressSummary()}`;
    setDocTitle();
    setExportEnabled(true);
    if (els.findBar && !els.findBar.hidden) runSearch(els.findInput.value);
    persistSoon();
}

function setExportEnabled(on) {
    els.shareBtn.disabled = !on;
    if (!on) hidePopoverEl(els.shareMenu);
}

function scheduleRender() {
    clearTimeout(renderTimer);
    renderTimer = setTimeout(updatePreview, 150);
}

let pendingEditorText = null;
function setMarkdown(text) {
    cancelFileImport();
    if (text.length > 1000000 && viewMode !== 'edit') {
        pendingEditorText = text;
        markdown = text;
        updatePreview();
        return;
    }
    pendingEditorText = null;
    if (window.__suEditorAPI) {
        window.__suEditorAPI.setText(text);   // triggers __suOnEditorChange -> render
    } else {
        window.__suInitialDoc = text;
        markdown = text;
        updatePreview();
    }
}

window.__suOnEditorChange = (text) => {
    cancelFileImport();
    markdown = text;
    persistSoon();
    scheduleRender();
};

window.__suEditorReady = () => {
    if (pendingEditorText !== null) return;
    if (window.__suInitialDoc) {
        markdown = window.__suInitialDoc;
        updatePreview();
    }
};

// ---- View mode (preview / source) ----
export function setViewMode(mode) {
    if (mode !== 'preview') { commitBlockEditor(); closeBlockMenu(); hideHandle(); hideFormatBar(); }
    viewMode = mode;
    document.body.classList.toggle('su-view-edit', mode === 'edit');
    for (const b of els.viewBtns) {
        const on = b.dataset.view === mode;
        b.classList.toggle('active', on);
        b.setAttribute('aria-pressed', on ? 'true' : 'false');
    }
    if (mode !== 'preview') closeFind();
    if (mode === 'edit') {
        if (pendingEditorText !== null && window.__suEditorAPI) {
            const text = pendingEditorText; pendingEditorText = null;
            window.__suEditorAPI.setText(text);
        }
        window.__suEditorAPI?.focus();
    }
}

for (const b of els.viewBtns) {
    b.addEventListener('click', () => {
        if (b.disabled) return;
        // Clicking the active Source segment returns to the preview.
        setViewMode(b.dataset.view === viewMode && viewMode !== 'preview' ? 'preview' : b.dataset.view);
    });
}

// ---- Fullscreen ----
els.fullscreenBtn.addEventListener('click', async () => {
    try {
        if (!document.fullscreenElement) await els.previewPane.requestFullscreen();
        else await document.exitFullscreen();
    } catch (e) { toast(`Fullscreen failed: ${e?.message ?? e}`); }
});
document.addEventListener('fullscreenchange', () => {
    const entering = !!document.fullscreenElement;
    els.fullscreenEnterIcon.style.display = entering ? 'none' : 'block';
    els.fullscreenExitIcon.style.display = entering ? 'block' : 'none';
});
document.addEventListener('click', async (e) => {
    const fig = e.target.closest?.('.su-mermaid-fullscreen')?.closest('.su-mermaid');
    if (!fig) return;
    try {
        if (document.fullscreenElement === fig) await document.exitFullscreen();
        else await fig.requestFullscreen();
    } catch (err) { toast(`Fullscreen failed: ${err?.message ?? err}`); }
});

// ---- Anchor links inside the preview (slug navigation + copy) ----
els.previewArticle.addEventListener('click', (e) => {
    const a = e.target.closest ? e.target.closest('a[href^="#"]') : null;
    if (!a) return;
    e.preventDefault();
    const id = decodeURIComponent(a.getAttribute('href').slice(1));
    const target = document.getElementById(id) || document.getElementById(a.getAttribute('href').slice(1));
    if (target) target.scrollIntoView({ block: 'start', behavior: 'smooth' });
    // The URL fragment holds the document itself, so anchors only scroll.
    const url = `${location.origin}${location.pathname}#${id}`;
    if (a.classList.contains('su-anchor')) {
        navigator.clipboard?.writeText(url).then(() => toast('Anchor link copied')).catch(() => {});
    }
});

// ---- Block editing in the preview ----
// Edits splice `markdown` at a block's source range (data-start/data-end from
// renderSourceBlocks), so untouched bytes stay identical. Headings act on their section.
const UNDO_LIMIT = 100;
const BLOCK_TARGET = 'li[data-start], .su-block[data-start]';
const undoStack = [];
const redoStack = [];
const expandedDone = new Set();
let hoverTarget = null;
let menuTarget = null;
let blockEditor = null;
let scrollAnchor = null;

const isHeadingBlock = (el) => !!el?.matches?.('.su-block[data-depth]');
const blockRange = (el) => [Number(el.dataset.start), Number(el.dataset.end)];
const headingKey = (block) => [...block.firstElementChild.childNodes]
    .filter((n) => !n.classList?.contains('su-anchor')).map((n) => n.textContent).join('').trim();
const isEditable = (el) => !!el?.closest?.('input, textarea, select, [contenteditable]:not([contenteditable="false"]), .cm-editor');

// The heading's next heading at the same or a higher level, or null at the end.
function sectionBoundary(heading) {
    const depth = Number(heading.dataset.depth);
    for (let el = heading.nextElementSibling; el; el = el.nextElementSibling) {
        if (isHeadingBlock(el) && Number(el.dataset.depth) <= depth) return el;
    }
    return null;
}
const sectionRange = (heading) => [Number(heading.dataset.start), Number(sectionBoundary(heading)?.dataset.start ?? markdown.length)];
const targetRange = (el) => (isHeadingBlock(el) ? sectionRange(el) : blockRange(el));

function previewInSync() {
    if (markdown === renderedMarkdown) return true;
    toast('The preview is updating. Try again in a moment.');
    return false;
}

function editSource(start, end, text, anchor) {
    undoStack.push(markdown);
    // ponytail: undo keeps full snapshots, so large documents keep fewer (~50M chars total).
    while (undoStack.length > Math.min(UNDO_LIMIT, Math.ceil(5e7 / markdown.length))) undoStack.shift();
    redoStack.length = 0;
    applyEdit(`${markdown.slice(0, start)}${text}${markdown.slice(end)}`, anchor);
}

function applyEdit(text, anchor) {
    const block = anchor?.closest?.('.su-block') ?? anchor;
    scrollAnchor = block?.isConnected ? { start: Number(block.dataset.start), top: block.getBoundingClientRect().top } : null;
    // Edits run inside user gestures, which lets Chromium show its write-permission prompt.
    fileHandle?.requestPermission?.({ mode: 'readwrite' }).then((state) => { if (state === 'granted') persistSoon(); }).catch(() => {});
    setMarkdown(text);
    // Preview edits (and their undo) re-render at once rather than after the typing debounce.
    clearTimeout(renderTimer);
    updatePreview();
}

function restoreScrollAnchor() {
    if (!scrollAnchor) return;
    const { start, top } = scrollAnchor;
    scrollAnchor = null;
    const block = [...els.previewArticle.querySelectorAll(':scope > .su-block')].find((b) => Number(b.dataset.start) >= start);
    if (block) els.previewScroll.scrollTop += block.getBoundingClientRect().top - top;
}

// ponytail: full-string snapshots (capped, so ~100x document size in memory).
// Switch to CodeMirror change sets if large documents need deep history.
function undoEdit(redo = false) {
    const [from, to] = redo ? [redoStack, undoStack] : [undoStack, redoStack];
    if (!from.length) { toast(redo ? 'Nothing to redo' : 'Nothing to undo'); return; }
    to.push(markdown);
    applyEdit(from.pop());
    toast(redo ? 'Redone' : 'Undone');
}

// Removes [start, end) and collapses the blank lines left at the seam.
function deleteSource(start, end, anchor) {
    const lead = markdown.slice(0, start).match(/\n*$/)[0].length;
    const trail = markdown.slice(end).match(/^\n*/)[0].length;
    const from = start - lead, to = end + trail;
    const seam = from === 0 ? 0 : to === markdown.length ? 1 : Math.min(lead + trail, 2);
    editSource(from, to, '\n'.repeat(seam), anchor);
}

function deleteTarget(el) {
    const [start, end] = targetRange(el);
    deleteSource(start, end, el);
    toast(isHeadingBlock(el) ? 'Section deleted' : 'Block deleted', { label: 'Undo', run: () => undoEdit() });
}

function toggleDone(el) {
    if (el.matches('li[data-check]')) {
        const at = Number(el.dataset.check) + 1;
        editSource(at, at + 1, markdown[at] === ' ' ? 'x' : ' ', el);
        return;
    }
    if (!isHeadingBlock(el)) return;
    const [start, end] = blockRange(el);
    const raw = markdown.slice(start, end);
    const lineEnd = raw.includes('\n') ? raw.indexOf('\n') : raw.length;
    const line = raw.slice(0, lineEnd);
    // ATX headings keep their markers; a setext heading's first line is its text.
    const [, prefix = '', text = line, suffix = ''] = line.match(/^( {0,3}#{1,6}[ \t]+)(.*?)([ \t]+#+[ \t]*)?$/) ?? [];
    const struck = text.match(/^~~(.*)~~$/);
    expandedDone.delete(struck ? struck[1] : text);
    editSource(start, start + lineEnd, `${prefix}${struck ? struck[1] : `~~${text}~~`}${suffix}`, el);
}

// Headings swap whole sections with a sibling section of the same level; other
// blocks and list items swap with their neighbour.
function siblingTarget(el, dir) {
    if (isHeadingBlock(el)) {
        const depth = Number(el.dataset.depth);
        if (dir > 0) {
            const next = sectionBoundary(el);
            return next && Number(next.dataset.depth) === depth ? next : null;
        }
        for (let prev = el.previousElementSibling; prev; prev = prev.previousElementSibling) {
            if (!isHeadingBlock(prev) || Number(prev.dataset.depth) > depth) continue;
            return Number(prev.dataset.depth) === depth ? prev : null;
        }
        return null;
    }
    const sibling = dir > 0 ? el.nextElementSibling : el.previousElementSibling;
    return sibling?.matches(el.matches('li') ? 'li[data-start]' : '.su-block[data-start]') ? sibling : null;
}

// Moves `el` past `count` siblings with chained swaps. A swap keeps the length of
// the span it touches, so sibling ranges read from the DOM stay valid throughout.
function moveTarget(el, dir, count = 1) {
    if (!siblingTarget(el, dir)) { toast(dir < 0 ? 'Already at the top' : 'Already at the bottom'); return; }
    const range = isHeadingBlock(el) ? sectionRange : blockRange;
    // Trailing newlines stay in place so separators survive the swap.
    const split = (s) => { const tail = s.match(/\n*$/)[0]; return [s.slice(0, s.length - tail.length), tail]; };
    let text = markdown, [s0, s1] = range(el), lo = s0, hi = s1;
    for (let other = el; count-- > 0 && (other = siblingTarget(other, dir));) {
        const [x0, x1, y0, y1] = dir < 0 ? [...range(other), s0, s1] : [s0, s1, ...range(other)];
        const [xCore, xTail] = split(text.slice(x0, x1)), [yCore, yTail] = split(text.slice(y0, y1));
        text = `${text.slice(0, x0)}${yCore}${xTail}${text.slice(x1, y0)}${xCore}${yTail}${text.slice(y1)}`;
        [s0, s1] = dir < 0 ? [x0, x0 + yCore.length + xTail.length] : [y1 - xCore.length - yTail.length, y1];
        lo = Math.min(lo, x0);
        hi = Math.max(hi, y1);
    }
    editSource(lo, hi, text.slice(lo, hi), el);
}

async function copyTarget(el) {
    els.previewArticle.classList.add('su-copying');
    const text = el.innerText.trim();
    els.previewArticle.classList.remove('su-copying');
    try { await navigator.clipboard.writeText(text); toast('Copied text'); } catch { toast('Failed to copy'); }
}

// Checkbox state, done sections (struck-through headings), and collapsing.
function decorateBlocks() {
    const root = els.previewArticle;
    for (const box of root.querySelectorAll('li[data-check] input[type="checkbox"]')) {
        const li = box.closest('li');
        if (!li.hasAttribute('data-check')) continue;
        box.disabled = false;
        box.title = 'Toggle done';
        li.classList.toggle('su-checked', box.checked);
    }
    let doneDepth = 0, collapsed = false;
    for (const el of root.children) {
        el.classList.remove('su-done', 'su-done-section', 'su-collapsed', 'su-expanded');
        const depth = isHeadingBlock(el) ? Number(el.dataset.depth) : 0;
        if (doneDepth && (!depth || depth > doneDepth)) {
            el.classList.add('su-done-section');
            el.classList.toggle('su-collapsed', collapsed);
            continue;
        }
        doneDepth = 0;
        if (!depth) continue;
        const key = headingKey(el);
        const del = el.firstElementChild.querySelector(':scope > del');
        if (!del || del.textContent.trim() !== key) continue;
        doneDepth = depth;
        collapsed = !expandedDone.has(key);
        el.classList.add('su-done', 'su-done-section');
        el.classList.toggle('su-expanded', !collapsed);
    }
}

// "N of M left" counts task items plus top-level sections (shallowest heading
// below h1). Shown once the document has tasks or a section is marked done.
function progressSummary() {
    const root = els.previewArticle;
    const tasks = [...root.querySelectorAll('li[data-check]')];
    const headings = [...root.querySelectorAll(':scope > .su-block[data-depth]')].filter((h) => h.dataset.depth !== '1');
    const depth = Math.min(...headings.map((h) => Number(h.dataset.depth)));
    const sections = headings.filter((h) => Number(h.dataset.depth) === depth);
    const doneSections = sections.filter((h) => h.classList.contains('su-done')).length;
    if (!tasks.length && !doneSections) return '';
    const left = tasks.filter((li) => !li.classList.contains('su-checked')).length + sections.length - doneSections;
    return ` · ${left} of ${tasks.length + sections.length} left`;
}

// ---- Block handle (+ and grip) and actions menu ----
function showHandle(target) {
    hoverTarget = target;
    const scroll = els.previewScroll.getBoundingClientRect();
    const rect = target.getBoundingClientRect();
    const indent = target.matches('li') ? 20 : 0; // clear the list marker
    els.blockHandle.style.top = `${rect.top - scroll.top + els.previewScroll.scrollTop}px`;
    els.blockHandle.style.left = `${Math.max(0, rect.left - scroll.left - indent - 50)}px`;
    els.blockHandle.hidden = false;
}
function hideHandle() {
    hoverTarget = null;
    els.blockHandle.hidden = true;
}
function closeBlockMenu() {
    els.blockMenu.hidden = true;
    els.blockGripBtn.setAttribute('aria-expanded', 'false');
    menuTarget = null;
}
function openBlockMenu() {
    if (!hoverTarget || !previewInSync()) return;
    menuTarget = hoverTarget;
    const heading = isHeadingBlock(menuTarget);
    const task = menuTarget.matches('li[data-check]');
    const done = heading ? menuTarget.classList.contains('su-done') : menuTarget.classList.contains('su-checked');
    const item = (action) => els.blockMenu.querySelector(`[data-action="${action}"]`);
    item('done').hidden = !heading && !task;
    item('done').textContent = done ? 'Mark not done' : 'Mark done';
    item('delete').textContent = heading ? 'Delete section' : 'Delete';
    item('up').textContent = heading ? 'Move section up' : 'Move up';
    item('down').textContent = heading ? 'Move section down' : 'Move down';
    const r = els.blockGripBtn.getBoundingClientRect();
    els.blockMenu.style.top = `${Math.round(Math.min(r.bottom + 4, window.innerHeight - 260))}px`;
    els.blockMenu.style.left = `${Math.round(r.left)}px`;
    els.blockMenu.hidden = false;
    els.blockGripBtn.setAttribute('aria-expanded', 'true');
    els.blockMenu.querySelector('button:not([hidden])')?.focus();
}

const blockActions = {
    edit: (el) => openBlockEditor(el),
    done: toggleDone,
    copy: copyTarget,
    up: (el) => moveTarget(el, -1),
    down: (el) => moveTarget(el, 1),
    delete: deleteTarget,
};

// A list's left padding (where the handle sits) belongs to the list block, so
// the item at the pointer's height wins; otherwise reaching for the handle
// would retarget it to the whole list.
function blockTargetAt(el, y) {
    const target = el.closest?.(BLOCK_TARGET);
    if (!target || !els.previewArticle.contains(target)) return null;
    const items = [target, ...target.querySelectorAll('li[data-start]')].filter((item) => {
        const r = item.getBoundingClientRect();
        return item.matches('li') && y >= r.top && y < r.bottom;
    });
    return items.at(-1) ?? (target.contains(hoverTarget) ? hoverTarget : target);
}
els.previewArticle.addEventListener('mousemove', (e) => {
    if (blockEditor || !els.blockMenu.hidden || drag) return;
    const target = blockTargetAt(e.target, e.clientY);
    if (target && target !== hoverTarget) showHandle(target);
});
els.previewScroll.addEventListener('mouseleave', (e) => {
    if (els.blockMenu.hidden && !drag && !els.blockHandle.contains(e.relatedTarget)) hideHandle();
});

// Dragging the grip reorders the block among the siblings Move up/down would
// visit; a plain click opens the menu.
let drag = null;
let gripDragged = false;
function dropPosition(source, y) {
    const list = [source];
    for (let s = siblingTarget(source, -1); s; s = siblingTarget(s, -1)) list.unshift(s);
    for (let s = siblingTarget(source, 1); s; s = siblingTarget(s, 1)) list.push(s);
    const visible = (el) => el.getClientRects().length > 0;
    let index = list.findIndex((el) => { if (!visible(el)) return false; const r = el.getBoundingClientRect(); return y < r.top + r.height / 2; });
    if (index < 0) index = list.length;
    const last = list.findLast(visible);
    const boundary = isHeadingBlock(last) && sectionBoundary(last);
    const lineY = index < list.length ? list[index].getBoundingClientRect().top
        : boundary ? boundary.getBoundingClientRect().top
        : (isHeadingBlock(last) ? [...els.previewArticle.children].findLast(visible) : last).getBoundingClientRect().bottom;
    const from = list.indexOf(source);
    return index > from ? { dir: 1, count: index - from - 1, lineY } : { dir: -1, count: from - index, lineY };
}
function endDrag() {
    drag?.source.classList.remove('su-dragging');
    els.blockDropLine.hidden = true;
    drag = null;
}
els.blockGripBtn.addEventListener('pointerdown', (e) => {
    gripDragged = false;
    if (e.button !== 0 || !hoverTarget) return;
    drag = { source: hoverTarget, x: e.clientX, y: e.clientY, active: false, drop: null };
    els.blockGripBtn.setPointerCapture(e.pointerId);
});
els.blockGripBtn.addEventListener('pointermove', (e) => {
    if (!drag) return;
    if (!drag.active) {
        if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 4 || !previewInSync()) return;
        drag.active = true;
        closeBlockMenu();
        drag.source.classList.add('su-dragging');
    }
    const scroll = els.previewScroll.getBoundingClientRect();
    // ponytail: edge scrolling only advances while the pointer moves.
    if (e.clientY < scroll.top + 40) els.previewScroll.scrollTop -= 20;
    else if (e.clientY > scroll.bottom - 40) els.previewScroll.scrollTop += 20;
    drag.drop = dropPosition(drag.source, e.clientY);
    const rect = drag.source.getBoundingClientRect();
    Object.assign(els.blockDropLine.style, {
        top: `${drag.drop.lineY - scroll.top + els.previewScroll.scrollTop - 1}px`,
        left: `${rect.left - scroll.left}px`,
        width: `${rect.width}px`,
    });
    els.blockDropLine.hidden = false;
});
els.blockGripBtn.addEventListener('pointerup', () => {
    const { active, source, drop } = drag ?? {};
    endDrag();
    if (!active) return;
    gripDragged = true;
    if (drop?.count && source.isConnected && previewInSync()) moveTarget(source, drop.dir, drop.count);
});
els.blockGripBtn.addEventListener('pointercancel', endDrag);
els.blockGripBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    if (gripDragged) { gripDragged = false; return; }
    if (els.blockMenu.hidden) openBlockMenu(); else closeBlockMenu();
});
els.blockAddBtn.addEventListener('click', () => { if (hoverTarget && previewInSync()) openBlockEditor(hoverTarget, true); });
els.blockMenu.addEventListener('click', (e) => {
    const action = e.target.closest('[data-action]')?.dataset.action;
    const target = menuTarget;
    closeBlockMenu();
    if (action && target?.isConnected && previewInSync()) blockActions[action](target);
});
document.addEventListener('click', (e) => {
    if (!els.blockMenu.hidden && !els.blockMenu.contains(e.target)) closeBlockMenu();
});
els.previewScroll.addEventListener('scroll', () => { if (!els.blockMenu.hidden) closeBlockMenu(); }, { passive: true });

els.previewArticle.addEventListener('click', (e) => {
    if (e.target.matches?.('input[type="checkbox"]')) {
        const li = e.target.closest('li');
        if (!li?.hasAttribute('data-check')) return;
        if (!previewInSync()) { e.preventDefault(); return; }
        toggleDone(li);
        return;
    }
    const heading = e.target.closest?.('.su-done > :is(h1, h2, h3, h4, h5, h6)');
    if (!heading || e.target.closest('a')) return;
    const key = headingKey(heading.parentElement);
    if (!expandedDone.delete(key)) expandedDone.add(key);
    decorateBlocks();
});

els.previewArticle.addEventListener('dblclick', (e) => {
    if (e.target.closest('a, input, button, .su-diagram, .su-mermaid, .su-block-editor')) return;
    const target = e.target.closest(BLOCK_TARGET);
    if (!target || !previewInSync()) return;
    window.getSelection()?.removeAllRanges();
    openBlockEditor(target);
});

// ---- Selection formatting toolbar ----
// A format wraps (or unwraps) exactly the selected characters in the block source.
const FORMATS = {
    bold: { tag: 'strong', markers: ['**', '__'] },
    italic: { tag: 'em', markers: ['*', '_'] },
    strike: { tag: 'del', markers: ['~~', '~'] },
    highlight: { tag: 'mark.su-hl', markers: ['=='] },
    code: { tag: 'code', markers: ['`'] },
};
let selectingWithMouse = false;
let formatTimer;
let pendingSelection = null;
let queuedFormat = null; // A format clicked while the previous one re-renders.

function selectionTarget() {
    const sel = window.getSelection();
    if (viewMode !== 'preview' || blockEditor || !sel?.rangeCount || sel.isCollapsed) return null;
    const range = sel.getRangeAt(0);
    const node = range.commonAncestorContainer;
    const el = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    const block = el?.closest(BLOCK_TARGET);
    if (!block || !els.previewArticle.contains(block) || el.closest('pre, .katex, .su-diagram, .su-mermaid, .su-anchor')) return null;
    const text = range.toString().trim();
    return text ? { range, el, block, text } : null;
}

// Rendered text of `block` up to a boundary point, without heading anchors.
function renderedText(block, container = block, offset = block.childNodes.length) {
    const range = document.createRange();
    range.setStart(block, 0);
    range.setEnd(container, offset);
    const frag = range.cloneContents();
    for (const anchor of frag.querySelectorAll('.su-anchor')) anchor.remove();
    return frag.textContent;
}

function inlineText(md) {
    const tpl = document.createElement('template');
    tpl.innerHTML = marked.parseInline(md);
    return tpl.content.textContent;
}

// Rendered characters are matched in order against the block source (skipping
// markup), then the span grows until it renders to exactly the text it covers,
// so markers inside it stay balanced. A selection that starts or ends inside a
// formatted run grows to cover the whole run. With `fits`, the span then widens over
// markup that renders nothing (`word` -> `~~word~~`) to the largest span `fits` accepts.
function selectionSourceRange({ range, block }, fits) {
    const [start, end] = blockRange(block);
    const raw = markdown.slice(start, end);
    const text = renderedText(block);
    let rs = renderedText(block, range.startContainer, range.startOffset).length;
    let re = renderedText(block, range.endContainer, range.endOffset).length;
    while (rs < re && /\s/.test(text[rs])) rs++;
    while (re > rs && /\s/.test(text[re - 1])) re--;
    // ponytail: greedy matching can land on markup that repeats the next character
    // (a link URL, say); the render check below rejects most such misses.
    const map = [];
    const space = /\s/g;
    const prefix = block.matches('li') ? raw.match(LIST_ITEM_RE)?.[0].length ?? 0 : 0;
    for (let i = 0, j = prefix; i < text.length; i++) {
        space.lastIndex = j;
        const k = /\s/.test(text[i]) ? (space.exec(raw)?.index ?? -1) : raw.indexOf(text[i], j);
        map.push(k);
        if (k >= 0) j = k + 1;
    }
    const a = map[rs], b = map[re - 1] + 1;
    if (re <= rs || a < 0 || b <= 0) return null;
    const covered = (s, e) => map.reduce((t, k, i) => (k >= s && k < e ? t + text[i] : t), '');
    // List continuations lose their indentation in the preview; compare visible whitespace,
    // but keep the original source offsets so formatting preserves every untouched byte.
    const normalize = (s) => s.replace(/\s+/g, ' ').trim();
    const renders = (s, e) => normalize(inlineText(raw.slice(s, e))) === normalize(covered(s, e));
    const widen = (s, e) => {
        const mapped = new Set(map);
        // ponytail: widening stops at 16 unrendered characters per side, enough for nested markers.
        let lo = s, hi = e;
        while (lo > 0 && s - lo < 16 && !mapped.has(lo - 1)) lo--;
        while (hi < raw.length && hi - e < 16 && !mapped.has(hi)) hi++;
        let best = [s, e];
        for (let x = lo; x <= s; x++) {
            for (let y = hi; y >= e; y--) {
                if (y - x > best[1] - best[0] && fits(start + x, start + y) && renders(x, y)) best = [x, y];
            }
        }
        return [start + best[0], start + best[1]];
    };
    // ponytail: growth stops at 64 characters; a longer miss shows the toast.
    for (let grow = 0; grow <= 64; grow++) {
        for (let before = Math.min(grow, a); before >= 0; before--) {
            const s = a - before, e = b + grow - before;
            if (e <= raw.length && renders(s, e)) return fits ? widen(s, e) : [start + s, start + e];
        }
    }
    return null;
}

// Wraps text in a marker, keeping edge whitespace outside it (`~~ a~~` is not strikethrough).
function wrapMarker(text, marker) {
    const core = text.trim();
    return core ? `${text.match(/^\s*/)[0]}${marker}${core}${marker}${text.match(/\s*$/)[0]}` : text;
}

// Runs that already have the format and touch the selection, with the source
// span of their contents and the marker around it.
function formatRuns(sel, format) {
    return [...sel.block.querySelectorAll(format.tag)].filter((el) => sel.range.intersectsNode(el)).flatMap((el) => {
        const contents = document.createRange();
        contents.selectNodeContents(el);
        // Markers go longest first, so `~~a~~` is not read as `~` around `~a~`.
        const between = (m) => (s, e) => markdown.slice(s - m.length, s) === m && markdown.startsWith(m, e);
        for (const marker of format.markers) {
            const span = selectionSourceRange({ range: contents, block: sel.block }, between(marker));
            if (span && between(marker)(...span)) return [{ start: span[0], end: span[1], marker }];
        }
        return [];
    });
}

// The bar stays up after a format: the re-render restores the selection from its
// rendered-text offsets, which formatting never changes.
function restoreFormatSelection() {
    const pending = pendingSelection;
    pendingSelection = null;
    const block = pending && els.previewArticle.querySelector(pending.selector);
    if (!block) return;
    const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, { acceptNode: (n) => (n.parentElement.closest('.su-anchor') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT) });
    const points = [];
    for (let pos = 0, node; points.length < 4 && (node = walker.nextNode()); pos += node.length) {
        if (!points.length && pos + node.length > pending.start) points.push(node, pending.start - pos);
        if (points.length && pos + node.length >= pending.end) points.push(node, pending.end - pos);
    }
    if (points.length !== 4) return;
    window.getSelection().setBaseAndExtent(...points);
    updateFormatBar();
}

function applyFormat(kind) {
    const sel = selectionTarget();
    if (!sel) { hideFormatBar(); return; }
    // The selection still marks the right text; apply once the preview catches up.
    if (markdown !== renderedMarkdown) { queuedFormat = kind; return; }
    const minimal = selectionSourceRange(sel);
    if (!minimal) { hideFormatBar(); toast('Could not find the selected text in the source. Double-click the block to edit it.'); return; }
    const format = FORMATS[kind];
    const runs = formatRuns(sel, format);
    // The selection takes in the markers of other formats it covers, staying inside a run of this format.
    const within = runs.find((r) => minimal[0] >= r.start && minimal[1] <= r.end);
    const [at, end] = selectionSourceRange(sel, within ? (s, e) => s >= within.start && e <= within.end : () => true);
    const text = markdown.slice(at, end);
    pendingSelection = {
        selector: `${sel.block.matches('li') ? 'li' : '.su-block'}[data-start="${sel.block.dataset.start}"]`,
        start: renderedText(sel.block, sel.range.startContainer, sel.range.startOffset).length,
        end: renderedText(sel.block, sel.range.endContainer, sel.range.endOffset).length,
    };
    // Inside a run that already has the format: remove it from the selection,
    // re-wrapping whatever is left of the run on either side.
    const outer = runs.find((r) => at >= r.start && end <= r.end);
    if (outer) {
        const m = outer.marker;
        editSource(outer.start - m.length, outer.end + m.length, `${wrapMarker(markdown.slice(outer.start, at), m)}${text}${wrapMarker(markdown.slice(end, outer.end), m)}`, sel.block);
        return;
    }
    // Runs inside the selection lose their markers, so the result is one run.
    const cuts = runs.filter((r) => r.start - r.marker.length >= at && r.end + r.marker.length <= end)
        .flatMap((r) => [[r.start - r.marker.length, r.start], [r.end, r.end + r.marker.length]])
        .sort((x, y) => x[0] - y[0]);
    let inner = '', pos = at;
    for (const [s, e] of cuts) { inner += markdown.slice(pos, s); pos = e; }
    inner += markdown.slice(pos, end);
    // ponytail: rendered text containing a backtick needs a longer code fence.
    editSource(at, end, wrapMarker(kind === 'code' ? inlineText(text) : inner, format.markers[0]), sel.block);
}

const COARSE_POINTER = matchMedia('(pointer: coarse)');
function hideFormatBar() { els.formatBar.hidden = true; }
function updateFormatBar() {
    const sel = selectionTarget();
    const range = sel?.range ?? commentableRange();
    if (!range) { hideFormatBar(); return; }
    // Selections across blocks or inside code can be commented on, not formatted.
    for (const el of els.formatBar.querySelectorAll('[data-format], [data-format-sep]')) el.hidden = !sel;
    const r = range.getBoundingClientRect();
    els.formatBar.hidden = false;
    const { width, height } = els.formatBar.getBoundingClientRect();
    // Touch browsers draw their own selection menu above the text, so ours goes below the handles.
    const below = COARSE_POINTER.matches ? r.bottom + 32 : r.top - height - 8 < 8 ? r.bottom + 8 : null;
    els.formatBar.style.top = `${Math.round(below !== null && below + height < window.innerHeight - 8 ? below : Math.max(8, r.top - height - 8))}px`;
    els.formatBar.style.left = `${Math.round(Math.min(Math.max(8, r.left + r.width / 2 - width / 2), window.innerWidth - width - 8))}px`;
}
document.addEventListener('selectionchange', () => {
    clearTimeout(formatTimer);
    if (!selectingWithMouse) formatTimer = setTimeout(updateFormatBar, 100);
});
// Only mouse drags defer the bar: a touch long-press selects text and ends in
// `pointercancel`, never `pointerup`.
els.previewArticle.addEventListener('pointerdown', (e) => { if (e.pointerType === 'mouse') selectingWithMouse = true; hideFormatBar(); });
const endPointerSelection = () => {
    if (!selectingWithMouse) return;
    selectingWithMouse = false;
    formatTimer = setTimeout(updateFormatBar, 0);
};
document.addEventListener('pointerup', endPointerSelection);
document.addEventListener('pointercancel', endPointerSelection);
els.previewScroll.addEventListener('scroll', () => { if (!els.formatBar.hidden) updateFormatBar(); }, { passive: true });
function formatBarAction(target) {
    if (target.closest('[data-comment]')) { startDraft(); return; }
    const kind = target.closest('[data-format]')?.dataset.format;
    if (kind) applyFormat(kind);
}
els.formatBar.addEventListener('mousedown', (e) => e.preventDefault());
// A tap would collapse the selection before `click`; cancelling touchstart keeps it
// (and suppresses that click), so taps act on pointerup instead.
els.formatBar.addEventListener('touchstart', (e) => e.preventDefault(), { passive: false });
els.formatBar.addEventListener('pointerup', (e) => { if (e.pointerType !== 'mouse') formatBarAction(e.target); });
els.formatBar.addEventListener('click', (e) => formatBarAction(e.target));

// ---- Review comments ----
// Threads anchor to the rendered text by quote plus surrounding context (like the
// W3C TextQuoteSelector), so the Markdown body stays byte-identical and formatting
// the quoted words keeps the anchor. Threads persist as one trailing HTML comment,
// which other Markdown renderers hide.
const COMMENTS_RE = /(?:^|\r?\n)<!-- su-comments\r?\n([\s\S]*?)\r?\n-->\s*$/;
const COMMENT_CONTEXT = 32;
const COMMENT_QUOTE_MAX = 2000;
const AUTHOR_KEY = 'su-md-author';
const RAIL_WIDTH_KEY = 'su-md-rail-width';
const RAIL_MODE_KEY = 'su-md-rail-mode';
const RAIL_MODES = ['expanded', 'collapsed', 'hidden'];
const RAIL_MODE_VERBS = { expanded: 'expand', collapsed: 'collapse', hidden: 'hide' };
const RAIL_NARROW = matchMedia('(max-width: 900px)');
const ICON = (paths) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;
const RESOLVE_SVG = ICON('<path d="M20 6 9 17l-5-5"/>');
const REOPEN_SVG = ICON('<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>');
const WIDEN_SVG = ICON('<path class="i-widen" d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/><path class="i-narrow" d="M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7"/>');
const TRASH_SVG = ICON('<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>');
const commentRenderer = new marked.Renderer();
commentRenderer.html = ({ text }) => escHTML(text);
const relativeTime = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
const escAttr = (s) => escHTML(String(s)).replace(/"/g, '&quot;');
const anchors = new Map(); // thread id -> Range in the preview, or null when the quote is gone
const highlights = 'highlights' in CSS
    ? Object.fromEntries(['su-comment', 'su-comment-active'].map((name) => { const h = new Highlight(); CSS.highlights.set(name, h); return [name, h]; }))
    : null;
let activeThread = null;
let draft = null; // A thread being composed: { id: 'draft', quote, prefix, suffix, comments: [] }
let railMode = 'expanded';
const collapsedOverrides = new Map(); // thread id -> collapsed, toggled from the card title
const isCollapsed = (t) => collapsedOverrides.get(t.id) ?? (t.id !== activeThread && (!!t.resolved || railMode === 'collapsed'));

const isThread = (t) => typeof t?.id === 'string' && typeof t.quote === 'string' && !!t.quote
    && Array.isArray(t.comments) && t.comments.length > 0 && t.comments.every((c) => typeof c?.body === 'string');

function setThreads(next) {
    threads = next;
    // `<` and `>` only occur inside JSON strings, so escaping them keeps `-->` out of the comment.
    commentsState = next.length ? JSON.stringify(next, null, 2).replace(/[<>]/g, (ch) => (ch === '<' ? '\\u003c' : '\\u003e')) : '';
}

function documentText(text = markdown) {
    const block = commentsState ? `${text.endsWith('\n') ? '' : '\n'}\n<!-- su-comments\n${commentsState}\n-->\n` : '';
    return withEol(`${frontmatter}${text}${block}`);
}

// Stable ids let shared copies merge their threads and comments back (see share/merge.ts).
const newId = () => crypto.randomUUID().slice(0, 8);
const authorName = (name) => (typeof name === 'string' && name.trim()) || 'Anonymous';
const savedAuthor = () => { try { return localStorage.getItem(AUTHOR_KEY) ?? ''; } catch { return ''; } };

function avatarHtml(name) {
    const full = authorName(name);
    const words = full.split(/\s+/);
    const initials = [...new Set([words[0], words.at(-1)])].map((w) => [...w][0]).join('').toUpperCase();
    let hue = 0;
    for (const ch of full) hue = (hue * 31 + ch.codePointAt(0)) % 360;
    return `<span class="cm-avatar" style="--hue:${hue}" aria-hidden="true">${escHTML(initials)}</span>`;
}

function timeAgo(date) {
    const ms = date - Date.now();
    for (const [unit, size] of [['year', 31536e6], ['month', 2592e6], ['week', 6048e5], ['day', 864e5], ['hour', 36e5], ['minute', 6e4]]) {
        if (Math.abs(ms) >= size) return relativeTime.format(Math.round(ms / size), unit);
    }
    return 'just now';
}

// Flattened preview text with the text node that starts at each offset.
function textIndex(root = els.previewArticle) {
    const nodes = [], starts = new Map();
    let text = '';
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => (node.parentElement.closest('.su-anchor, .su-block-editor, .katex-mathml, style, script') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
    });
    for (let node; (node = walker.nextNode());) { starts.set(node, text.length); nodes.push(node); text += node.data; }
    return { text, nodes, starts };
}

function textOffset(index, node, offset) {
    if (index.starts.has(node)) return index.starts.get(node) + offset;
    // An element boundary (or skipped text): the next indexed text node starts there.
    const point = document.createRange();
    point.setStart(node, offset);
    const next = index.nodes.find((n) => point.comparePoint(n, 0) >= 0);
    return next ? index.starts.get(next) : index.text.length;
}

function textPoint(index, pos, end = false) {
    const node = index.nodes.findLast((n) => (end ? index.starts.get(n) < pos : index.starts.get(n) <= pos)) ?? index.nodes[0];
    return [node, pos - index.starts.get(node)];
}

const sharedLength = (a, b, fromEnd = false) => {
    let n = 0;
    while (n < a.length && n < b.length && (fromEnd ? a[a.length - 1 - n] === b[b.length - 1 - n] : a[n] === b[n])) n++;
    return n;
};

// ponytail: exact quote match only; any edit inside the quoted text detaches the
// thread (shown struck through). Add fuzzy matching if detaching becomes common.
function locate(index, thread) {
    const { text } = index, { quote, prefix = '', suffix = '' } = thread;
    let best = -1, bestScore = -1;
    for (let i = text.indexOf(quote); i >= 0; i = text.indexOf(quote, i + 1)) {
        const end = i + quote.length;
        const score = sharedLength(text.slice(Math.max(0, i - COMMENT_CONTEXT), i), prefix, true) + sharedLength(text.slice(end, end + COMMENT_CONTEXT), suffix);
        if (score > bestScore) [best, bestScore] = [i, score];
    }
    if (best < 0) return null;
    const range = document.createRange();
    range.setStart(...textPoint(index, best));
    range.setEnd(...textPoint(index, best + quote.length, true));
    return range;
}

const allThreads = () => (draft ? [...threads, draft] : threads);
const railHidden = () => document.body.classList.contains('su-rail-hidden');
const threadCard = (id) => els.commentsRail.querySelector(`.cm-card[data-id="${CSS.escape(id)}"]`);

function anchorThreads() {
    anchors.clear();
    if (allThreads().length && renderedMarkdown.trim()) {
        const index = textIndex();
        for (const t of allThreads()) anchors.set(t.id, locate(index, t) ?? threadHooks.fallbackAnchor?.(t) ?? null);
    }
    updateHighlights();
}

function updateHighlights() {
    if (!highlights) return;
    for (const h of Object.values(highlights)) h.clear();
    if (railHidden()) return;
    for (const t of allThreads()) {
        const range = anchors.get(t.id);
        if (range && !t.resolved) highlights[t.id === activeThread ? 'su-comment-active' : 'su-comment'].add(range);
    }
}

function commentableRange() {
    const sel = window.getSelection();
    if (viewMode !== 'preview' || blockEditor || !sel?.rangeCount || sel.isCollapsed) return null;
    const range = sel.getRangeAt(0);
    return els.previewArticle.contains(range.commonAncestorContainer) && range.toString().trim() ? range : null;
}

function startDraft() {
    const range = commentableRange();
    if (!range || !previewInSync()) return;
    const index = textIndex();
    let start = textOffset(index, range.startContainer, range.startOffset);
    let end = textOffset(index, range.endContainer, range.endOffset);
    while (start < end && /\s/.test(index.text[start])) start++;
    while (end > start && /\s/.test(index.text[end - 1])) end--;
    if (end <= start) return;
    if (end - start > COMMENT_QUOTE_MAX) { toast('Select a shorter passage to comment on'); return; }
    window.getSelection().removeAllRanges();
    hideFormatBar();
    draft = {
        id: 'draft',
        quote: index.text.slice(start, end),
        prefix: index.text.slice(Math.max(0, start - COMMENT_CONTEXT), start),
        suffix: index.text.slice(end, end + COMMENT_CONTEXT),
        comments: [],
    };
    activeThread = draft.id;
    if (railMode === 'hidden') setRailMode(savedRailMode());
    anchorThreads();
    renderRail();
    const card = threadCard(draft.id);
    card?.querySelector('textarea').focus({ preventScroll: true });
    if (RAIL_NARROW.matches) card?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
}

function discardDraft() {
    draft = null;
    if (activeThread === 'draft') activeThread = null;
    anchorThreads();
    renderRail();
}

const commentHtml = (c, i) => {
    const date = new Date(c.at);
    const dated = Number.isFinite(date.getTime());
    return `<div class="cm-comment" data-index="${i}">
<div class="cm-meta">${avatarHtml(c.author)}<span class="cm-name">${escHTML(authorName(c.author))}</span>${threadHooks.isNew?.(c) ? '<span class="cm-tag cm-new">New</span>' : ''}${dated ? `<time class="cm-time" datetime="${date.toISOString()}" title="${escAttr(date.toLocaleString())}">${timeAgo(date)}${c.edited ? ' · edited' : ''}</time>` : ''}<span class="cm-actions"><button type="button" data-act="edit">Edit</button>${i ? '<button type="button" data-act="delete">Delete</button>' : ''}</span></div>
<div class="cm-body preview-article"></div>
</div>`;
};

const composerHtml = (label, text = '') => `<form class="cm-composer">
<textarea rows="1" placeholder="${label === 'Reply' ? 'Reply…' : 'Comment… Markdown works'}" aria-label="${label}">${escHTML(text)}</textarea>
<div class="cm-foot">${avatarHtml(savedAuthor())}<input class="cm-author" value="${escAttr(savedAuthor())}" placeholder="Your name" aria-label="Your name" maxlength="60" autocomplete="nickname"><button type="button" class="btn btn-sm" data-act="cancel">Cancel</button><button type="submit" class="btn btn-primary btn-sm">${label}</button></div>
</form>`;

// Comments use the document pipeline (Mermaid, models, maps, math, alerts) through its
// static export output, which renders once per body and rehydrates on every rebuild.
// ponytail: unbounded cache keyed by comment text; fine for a review session.
const renderedComments = new Map(); // comment Markdown -> HTML, or a Promise while rendering
function fillCommentBody(body, md) {
    const cached = renderedComments.get(md);
    if (typeof cached === 'string') {
        body.innerHTML = cached;
        initDiagrams(body);
    } else {
        body.innerHTML = marked.parse(md, { renderer: commentRenderer });
        for (const code of body.querySelectorAll('pre code')) { try { hljs.highlightElement(code); } catch {} }
        const pending = cached ?? render(md, true, true).then((html) => { renderedComments.set(md, html); return html; });
        renderedComments.set(md, pending);
        pending.then(() => { if (body.isConnected) fillCommentBody(body, md); }, () => renderedComments.delete(md));
    }
    for (const a of body.querySelectorAll('a[href]')) {
        if (!/^(?:https?:|mailto:|#)/i.test(a.getAttribute('href'))) a.removeAttribute('href');
        a.target = '_blank';
        a.rel = 'noopener noreferrer';
    }
}

function buildCard(t) {
    const card = document.createElement('section');
    const isDraft = t === draft, anchored = !!anchors.get(t.id);
    card.className = `cm-card${isDraft ? ' draft' : ''}${t.resolved ? ' resolved' : ''}${anchored ? '' : ' orphan'}`;
    card.dataset.id = t.id;
    card.setAttribute('aria-label', `Comments on “${t.quote.slice(0, 80)}”`);
    const actions = isDraft ? '' : `${t.resolved ? '<span class="cm-tag">Resolved</span>' : ''}
<button type="button" class="icon-btn" data-act="resolve" title="${t.resolved ? 'Reopen' : 'Resolve'}" aria-label="${t.resolved ? 'Reopen thread' : 'Resolve thread'}">${t.resolved ? REOPEN_SVG : RESOLVE_SVG}</button>
<button type="button" class="icon-btn cm-widen" data-act="widen" title="Widen or narrow comments" aria-label="Widen or narrow comments">${WIDEN_SVG}</button>
<button type="button" class="icon-btn" data-act="delete-thread" title="Delete thread" aria-label="Delete thread">${TRASH_SVG}</button>`;
    const authors = [...new Set(t.comments.map((c) => authorName(c.author)))];
    const summary = `<span class="cm-summary" aria-hidden="true">${authors.slice(0, 3).map(avatarHtml).join('')}${t.comments.length > 1 ? `<span class="cm-count">${t.comments.length}</span>` : ''}</span>`;
    const note = isDraft ? '' : threadHooks.note?.(t);
    card.innerHTML = `<div class="cm-head"><button type="button" class="cm-quote" data-act="toggle" title="${anchored ? 'Collapse or expand' : 'The commented text was changed or removed'}">${escHTML(t.quote)}</button>${summary}${actions}</div>
${note ? `<div class="cm-note">${escHTML(note)}</div>` : ''}
${t.comments.map(commentHtml).join('')}
${composerHtml(isDraft ? 'Comment' : 'Reply')}`;
    for (const [i, body] of card.querySelectorAll('.cm-body').entries()) fillCommentBody(body, t.comments[i].body);
    syncCard(card, t);
    return card;
}

function syncCard(card, t = allThreads().find((x) => x.id === card.dataset.id)) {
    const collapsed = !!t && isCollapsed(t);
    card.classList.toggle('active', card.dataset.id === activeThread);
    card.classList.toggle('collapsed', collapsed);
    card.querySelector('.cm-quote').setAttribute('aria-expanded', String(!collapsed));
}

const fitTextarea = (ta) => { ta.style.height = 'auto'; ta.style.height = `${ta.scrollHeight + 2}px`; };

function renderRail() {
    const all = allThreads();
    document.body.classList.toggle('su-has-comments', all.length > 0 && !!markdown.trim());
    const open = threads.filter((t) => !t.resolved).length;
    els.commentsBtn.hidden = !threads.length;
    els.commentsCount.textContent = open || '';
    const next = RAIL_MODES[(RAIL_MODES.indexOf(railMode) + 1) % RAIL_MODES.length];
    els.commentsBtn.dataset.mode = railMode;
    els.commentsBtn.title = `Comments ${railMode} (${open} open). Click to ${RAIL_MODE_VERBS[next]}.`;
    els.commentsBtn.setAttribute('aria-label', els.commentsBtn.title);
    // Unsent replies survive rebuilds triggered elsewhere.
    const pending = [...els.commentsRail.querySelectorAll('.cm-composer:not([data-edit]) textarea')]
        .filter((ta) => ta.value).map((ta) => [ta.closest('.cm-card').dataset.id, ta.value]);
    for (const fig of els.commentsRail.querySelectorAll('.su-diagram')) fig.__suDispose?.();
    els.commentsRail.replaceChildren(els.railResizer, ...all.map(buildCard));
    for (const [id, value] of pending) {
        const ta = threadCard(id)?.querySelector('.cm-composer textarea');
        if (ta) ta.value = value;
    }
    railObserver.disconnect();
    railObserver.observe(els.previewArticle);
    for (const card of els.commentsRail.querySelectorAll('.cm-card')) railObserver.observe(card);
    for (const ta of els.commentsRail.querySelectorAll('textarea')) if (ta.value) fitTextarea(ta);
    layoutRail();
}

// Cards sit beside their highlighted text. The active card stays level with its
// text and the others move out of its way; detached threads go last.
function layoutRail() {
    const cards = [...els.commentsRail.querySelectorAll('.cm-card')];
    if (RAIL_NARROW.matches || !cards.length || railHidden()) {
        for (const card of cards) card.style.top = '';
        els.commentsRail.style.minHeight = '';
        return;
    }
    const base = els.commentsRail.getBoundingClientRect().top;
    const items = cards.map((card) => {
        const rect = anchors.get(card.dataset.id)?.getClientRects()[0];
        return { card, y: rect ? rect.top - base : Infinity, h: card.offsetHeight };
    }).sort((a, b) => a.y - b.y);
    const pos = items.map((item) => (Number.isFinite(item.y) ? item.y : 0));
    const active = items.findIndex((item) => item.card.dataset.id === activeThread);
    for (let i = active - 1; i >= 0; i--) pos[i] = Math.min(pos[i], pos[i + 1] - items[i].h - 10);
    let floor = 0;
    for (const [i, item] of items.entries()) {
        pos[i] = Math.max(pos[i], floor);
        item.card.style.top = `${pos[i]}px`;
        floor = pos[i] + item.h + 10;
    }
    els.commentsRail.style.minHeight = `${floor}px`;
}
let railFrame = 0;
const railObserver = new ResizeObserver(() => { cancelAnimationFrame(railFrame); railFrame = requestAnimationFrame(layoutRail); });

function activateThread(id) {
    const emptyDraft = draft && id !== draft.id && !threadCard(draft.id)?.querySelector('textarea').value.trim();
    activeThread = id;
    collapsedOverrides.delete(id);
    if (emptyDraft) { discardDraft(); return; }
    for (const card of els.commentsRail.querySelectorAll('.cm-card')) syncCard(card);
    updateHighlights();
    layoutRail();
}

function commentsChanged() {
    setThreads(threads);
    anchorThreads();
    renderRail();
    persistSoon();
}

function changeThreads(change, message) {
    const before = structuredClone(threads);
    change();
    commentsChanged();
    if (message) toast(message, { label: 'Undo', run: () => { threads = before; commentsChanged(); } });
}

function setRailWidth(px) {
    const width = Math.round(Math.min(Math.max(px, 240), window.innerWidth * 0.75));
    document.documentElement.style.setProperty('--su-rail-w', `${width}px`);
    return width;
}
try { const width = Number(localStorage.getItem(RAIL_WIDTH_KEY)); if (width) setRailWidth(width); } catch {}

function toggleRailWide() {
    document.body.classList.toggle('su-rail-wide');
    layoutRail();
}

els.railResizer.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    els.railResizer.setPointerCapture(e.pointerId);
    const startX = e.clientX, startWidth = els.commentsRail.offsetWidth;
    document.body.classList.remove('su-rail-wide');
    els.railResizer.onpointermove = (ev) => {
        const width = setRailWidth(startWidth + startX - ev.clientX);
        try { localStorage.setItem(RAIL_WIDTH_KEY, String(width)); } catch {}
    };
});
els.railResizer.addEventListener('pointerup', () => { els.railResizer.onpointermove = null; });
els.railResizer.addEventListener('dblclick', toggleRailWide);

// Hidden is a momentary choice; only expanded/collapsed are remembered.
const savedRailMode = () => { try { return localStorage.getItem(RAIL_MODE_KEY) === 'collapsed' ? 'collapsed' : 'expanded'; } catch { return 'expanded'; } };
function setRailMode(mode) {
    railMode = mode;
    collapsedOverrides.clear();
    document.body.classList.toggle('su-rail-hidden', mode === 'hidden');
    if (mode !== 'hidden') try { localStorage.setItem(RAIL_MODE_KEY, mode); } catch {}
}
railMode = savedRailMode();

els.commentsBtn.addEventListener('click', () => {
    setRailMode(RAIL_MODES[(RAIL_MODES.indexOf(railMode) + 1) % RAIL_MODES.length]);
    renderRail();
    updateHighlights();
});

// Clicking highlighted text opens its thread; the shortest match wins when threads overlap.
els.previewArticle.addEventListener('click', (e) => {
    if (!allThreads().length || railHidden() || !window.getSelection()?.isCollapsed || e.target.closest('a, input, button')) return;
    const caret = document.caretPositionFromPoint?.(e.clientX, e.clientY);
    const range = caret ? null : document.caretRangeFromPoint?.(e.clientX, e.clientY);
    const [node, offset] = caret ? [caret.offsetNode, caret.offset] : [range?.startContainer, range?.startOffset];
    let hit = null, hitLength = Infinity;
    for (const t of threads) {
        const r = anchors.get(t.id);
        if (t.resolved || !r || !node || !r.isPointInRange(node, offset)) continue;
        const length = r.toString().length;
        if (length < hitLength) [hit, hitLength] = [t.id, length];
    }
    if (hit === activeThread) return;
    activateThread(hit);
    if (hit && RAIL_NARROW.matches) threadCard(hit)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
});

els.commentsRail.addEventListener('click', (e) => {
    const card = e.target.closest('.cm-card');
    if (!card) return;
    const id = card.dataset.id;
    const thread = threads.find((t) => t.id === id);
    const act = e.target.closest('[data-act]')?.dataset.act;
    const wasCollapsed = card.classList.contains('collapsed');
    if (id !== activeThread && act !== 'cancel') activateThread(id);
    if (!card.isConnected) return; // Activating discarded an empty draft and rebuilt the rail.
    const comment = e.target.closest('.cm-comment');
    const index = Number(comment?.dataset.index);
    if (!act && card.classList.contains('collapsed')) { collapsedOverrides.delete(id); syncCard(card); }
    if (act === 'toggle') {
        if (id !== 'draft') { collapsedOverrides.set(id, !wasCollapsed); syncCard(card); }
        if (!card.classList.contains('collapsed')) anchors.get(id)?.startContainer.parentElement.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    } else if (act === 'widen') toggleRailWide();
    else if (act === 'resolve') {
        changeThreads(() => { if (thread.resolved) delete thread.resolved; else thread.resolved = true; }, thread.resolved ? null : 'Thread resolved');
        if (thread.resolved) activateThread(null);
    } else if (act === 'delete-thread') changeThreads(() => threads.splice(threads.indexOf(thread), 1), 'Thread deleted');
    else if (act === 'delete') changeThreads(() => thread.comments.splice(index, 1), 'Comment deleted');
    else if (act === 'edit') {
        const tpl = document.createElement('template');
        tpl.innerHTML = composerHtml('Save', thread.comments[index].body);
        const form = tpl.content.firstElementChild;
        form.dataset.edit = String(index);
        comment.querySelector('.cm-body').replaceWith(form);
        const ta = form.querySelector('textarea');
        fitTextarea(ta);
        ta.focus();
    } else if (act === 'cancel') {
        const form = e.target.closest('form');
        if (id === 'draft') discardDraft();
        else if (form.dataset.edit) renderRail();
        else { form.querySelector('textarea').value = ''; e.target.blur(); }
    }
});

els.commentsRail.addEventListener('submit', (e) => {
    e.preventDefault();
    const form = e.target, ta = form.querySelector('textarea');
    const body = ta.value.trim();
    if (!body) { ta.focus(); return; }
    const author = form.querySelector('.cm-author').value.trim();
    try { if (author) localStorage.setItem(AUTHOR_KEY, author); } catch {}
    const at = new Date().toISOString();
    const id = form.closest('.cm-card').dataset.id;
    const thread = threads.find((t) => t.id === id);
    ta.value = '';
    if (form.dataset.edit) {
        Object.assign(thread.comments[Number(form.dataset.edit)], { body, edited: at });
    } else if (id === 'draft') {
        const { quote, prefix, suffix } = draft;
        const created = { id: newId(), quote, prefix, suffix, comments: [{ id: newId(), author: authorName(author), at, body }] };
        // Keep threads in document order, so the stored block reads top to bottom.
        const range = anchors.get('draft');
        const next = range ? threads.findIndex((t) => anchors.get(t.id)?.compareBoundaryPoints(Range.START_TO_START, range) > 0) : -1;
        threads.splice(next < 0 ? threads.length : next, 0, created);
        draft = null;
        activeThread = created.id;
    } else {
        thread.comments.push({ id: newId(), author: authorName(author), at, body });
    }
    commentsChanged();
});

els.commentsRail.addEventListener('input', (e) => {
    if (e.target.matches('textarea')) fitTextarea(e.target);
    else if (e.target.matches('.cm-author')) e.target.previousElementSibling.outerHTML = avatarHtml(e.target.value);
});

els.commentsRail.addEventListener('keydown', (e) => {
    const form = e.target.closest?.('.cm-composer');
    if (!form) return;
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); form.requestSubmit(); }
    else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); form.querySelector('[data-act="cancel"]').click(); }
});

// Clicking the empty space below the last block starts a new block at the end.
els.previewScroll.addEventListener('click', (e) => {
    if (e.target !== els.previewScroll && e.target !== els.previewArticle) return;
    const last = els.previewArticle.lastElementChild;
    if (!markdown.trim() || viewMode !== 'preview' || !last || e.clientY < last.getBoundingClientRect().bottom) return;
    if (previewInSync()) openBlockEditor(null, true);
});

// ---- Inline block editor ----
function createTextareaEditor(parent, text) {
    const ta = document.createElement('textarea');
    ta.className = 'editor-fallback';
    ta.spellcheck = false;
    ta.value = text;
    const fit = () => { ta.style.height = 'auto'; ta.style.height = `${ta.scrollHeight}px`; };
    ta.addEventListener('input', fit);
    parent.append(ta);
    requestAnimationFrame(fit);
    return {
        getText: () => ta.value,
        focus: () => { ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length); },
        destroy: () => {},
    };
}

// Insert mode places new text after `target` (or at the end of the document).
// List items prefill a sibling marker; other blocks start empty.
function openBlockEditor(target, insert = false) {
    commitBlockEditor();
    closeBlockMenu();
    hideHandle();
    let start, end, text = '', tail = '';
    if (insert) {
        start = end = target ? Number(target.dataset.end) : markdown.length;
        const marker = target?.matches('li') && markdown.slice(...blockRange(target)).match(LIST_ITEM_RE);
        if (marker) text = `${marker[1].replace(/\d+/, (n) => Number(n) + 1)}${marker[2] ? '[ ] ' : ''}`;
    } else {
        [start, end] = blockRange(target);
        const raw = markdown.slice(start, end);
        text = raw.replace(/\n+$/, '');
        tail = raw.slice(text.length);
    }
    const wrap = document.createElement('div');
    wrap.className = 'su-block-editor';
    if (target) target.after(wrap); else els.previewArticle.append(wrap);
    if (target && !insert) target.hidden = true;
    const editor = window.__suCreateBlockEditor?.(wrap, text) ?? createTextareaEditor(wrap, text);
    blockEditor = { wrap, target, insert, start, end, text, tail, editor, listItem: !!target?.matches('li') };
    wrap.addEventListener('keydown', (e) => {
        if (e.key !== 'Escape' && !(e.key === 'Enter' && (e.metaKey || e.ctrlKey))) return;
        e.preventDefault();
        e.stopPropagation();
        commitBlockEditor();
    }, true);
    wrap.addEventListener('focusout', (e) => { if (!wrap.contains(e.relatedTarget)) commitBlockEditor(); });
    wrap.scrollIntoView({ block: 'nearest' });
    editor.focus();
}

// Separates inserted text from its neighbours: list items join the list, other
// blocks get blank lines on both sides.
function insertionText(pos, value, listItem) {
    const before = markdown.slice(0, pos), after = markdown.slice(pos);
    if (listItem) return before.endsWith('\n') ? `${value}\n` : `\n${value}`;
    const lead = !before || before.endsWith('\n\n') ? '' : before.endsWith('\n') ? '\n' : '\n\n';
    const trail = !after || after.startsWith('\n') ? '\n' : '\n\n';
    return `${lead}${value}${trail}`;
}

function commitBlockEditor() {
    const ed = blockEditor;
    if (!ed) return;
    blockEditor = null;
    const value = ed.editor.getText();
    ed.editor.destroy();
    ed.wrap.remove();
    if (ed.target) ed.target.hidden = false;
    // Unchanged or emptied blocks are no-ops; Delete in the menu removes a block.
    if (!value.trim() || value === ed.text || markdown !== renderedMarkdown) return;
    if (ed.insert) editSource(ed.start, ed.start, insertionText(ed.start, value, ed.listItem), ed.target);
    else editSource(ed.start, ed.end, `${value}${ed.tail}`, ed.target);
}

els.optHideDone.addEventListener('change', () => {
    els.previewArticle.classList.toggle('su-hide-done', els.optHideDone.checked);
    hideHandle();
});

// ---- Find in document ----
let searchHits = [];
let currentHit = -1;
function clearSearch() {
    for (const mark of searchHits) {
        const parent = mark.parentNode;
        if (!parent) continue;
        parent.replaceChild(document.createTextNode(mark.textContent), mark);
        parent.normalize();
    }
    searchHits = [];
    currentHit = -1;
    updateSearchCount();
    // Rewrapping text nodes collapses the comment ranges inside them.
    anchorThreads();
}
function updateSearchCount() {
    els.findCount.textContent = searchHits.length ? `${currentHit + 1}/${searchHits.length}` : '0/0';
}
function selectHit(index) {
    if (!searchHits.length) { updateSearchCount(); return; }
    if (currentHit >= 0 && searchHits[currentHit]) searchHits[currentHit].classList.remove('current');
    currentHit = (index + searchHits.length) % searchHits.length;
    const hit = searchHits[currentHit];
    hit.classList.add('current');
    hit.scrollIntoView({ block: 'center', inline: 'nearest' });
    updateSearchCount();
}
function runSearch(query) {
    clearSearch();
    query = String(query || '').trim();
    if (!query) return;
    const needle = query.toLowerCase();
    const walker = document.createTreeWalker(els.previewArticle, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => {
            if (!node.nodeValue || node.nodeValue.toLowerCase().indexOf(needle) < 0) return NodeFilter.FILTER_REJECT;
            const parent = node.parentElement;
            if (!parent || parent.closest('script,style,svg,mark.find-hit,.katex,.su-mermaid,.su-diagram,.su-anchor')) return NodeFilter.FILTER_REJECT;
            return NodeFilter.FILTER_ACCEPT;
        },
    });
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const node of nodes) {
        const text = node.nodeValue;
        const lower = text.toLowerCase();
        const frag = document.createDocumentFragment();
        let start = 0, index;
        while ((index = lower.indexOf(needle, start)) >= 0) {
            if (index > start) frag.appendChild(document.createTextNode(text.slice(start, index)));
            const mark = document.createElement('mark');
            mark.className = 'find-hit';
            mark.textContent = text.slice(index, index + query.length);
            searchHits.push(mark);
            frag.appendChild(mark);
            start = index + query.length;
        }
        if (start < text.length) frag.appendChild(document.createTextNode(text.slice(start)));
        node.parentNode.replaceChild(frag, node);
    }
    anchorThreads();
    selectHit(0);
}
function openFind() {
    if (viewMode !== 'preview') setViewMode('preview');
    if (!markdown.trim()) return;
    els.findBar.hidden = false;
    els.findInput.focus();
    els.findInput.select();
    if (els.findInput.value) runSearch(els.findInput.value);
}
function closeFind() {
    els.findBar.hidden = true;
    clearSearch();
}
els.findBtn.addEventListener('click', () => (els.findBar.hidden ? openFind() : closeFind()));
els.findClose.addEventListener('click', closeFind);
els.findPrev.addEventListener('click', () => selectHit(currentHit - 1));
els.findNext.addEventListener('click', () => selectHit(currentHit + 1));
els.findInput.addEventListener('input', () => runSearch(els.findInput.value));
els.findInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); selectHit(currentHit + (e.shiftKey ? -1 : 1)); }
    else if (e.key === 'Escape') { e.preventDefault(); closeFind(); els.previewScroll.focus?.(); }
});

// ---- Popover tether (native popover + commandfor), with fallback ----
const hidePopoverEl = (el) => {
    if (typeof el.hidePopover === 'function') { if (el.matches(':popover-open')) el.hidePopover(); }
    else el.hidden = true;
};
const positionPopover = (popover, invoker) => {
    if (typeof popover.showPopover !== 'function') return;
    const r = invoker.getBoundingClientRect();
    popover.style.position = 'fixed';
    popover.style.top = `${Math.round(r.bottom + 6)}px`;
    popover.style.left = 'auto';
    popover.style.right = `${Math.round(window.innerWidth - r.right)}px`;
    popover.style.bottom = 'auto';
};
const wireTether = (invoker, popover) => {
    invoker.addEventListener('click', () => positionPopover(popover, invoker));
    popover.addEventListener('beforetoggle', (e) => { if (e.newState === 'open') positionPopover(popover, invoker); });
};
wireTether(els.optionsBtn, els.optionsPopover);
wireTether(els.shareBtn, els.shareMenu);

if (!('popover' in HTMLElement.prototype) || !('commandForElement' in HTMLButtonElement.prototype)) {
    const pops = document.querySelectorAll('[popover]');
    for (const p of pops) { p.hidden = true; p.style.position = 'absolute'; p.style.top = 'calc(100% + 6px)'; p.style.right = '0'; }
    const setState = (p, open) => { if (p.hidden === !open) return; p.hidden = !open; p.dispatchEvent(Object.assign(new Event('toggle'), { newState: open ? 'open' : 'closed' })); };
    const closeAll = () => { for (const p of pops) setState(p, false); for (const i of document.querySelectorAll('[commandfor]')) i.setAttribute('aria-expanded', 'false'); };
    for (const inv of document.querySelectorAll('[commandfor]')) {
        const t = document.getElementById(inv.getAttribute('commandfor'));
        if (!t) continue;
        inv.addEventListener('click', (e) => { if (inv.disabled) return; e.stopPropagation(); const opening = t.hidden; closeAll(); if (opening) { setState(t, true); inv.setAttribute('aria-expanded', 'true'); } });
    }
    document.addEventListener('click', (e) => { for (const p of pops) if (!p.hidden && !p.contains(e.target)) closeAll(); });
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeAll(); });
}

const READING_WIDTH_MIN = 320;
const READING_WIDTH_MAX = 2400;
let customReadingWidth = 800;

function normalizeReadingWidth(value) {
    if (value === 'full') return 'full';
    const width = Number(value);
    return Number.isInteger(width) && width >= READING_WIDTH_MIN && width <= READING_WIDTH_MAX ? width : 800;
}

function readReadingWidth() {
    return els.optReadingWidth.value === 'custom' ? customReadingWidth : normalizeReadingWidth(els.optReadingWidth.value);
}

function applyReadingWidth() {
    const width = readReadingWidth();
    document.documentElement.style.setProperty('--su-reading-width', width === 'full' ? 'none' : `${width}px`);
    document.documentElement.style.setProperty('--su-reading-track', width === 'full' ? '1fr' : `${width}px`);
}

els.optReadingWidth.addEventListener('change', () => {
    els.optReadingCustomWrap.hidden = els.optReadingWidth.value !== 'custom';
    applyReadingWidth();
    if (!els.optReadingCustomWrap.hidden) els.optReadingCustom.focus();
});
els.optReadingCustom.addEventListener('input', () => {
    const width = Number(els.optReadingCustom.value);
    if (els.optReadingCustom.value.trim() && Number.isInteger(width) && width >= READING_WIDTH_MIN && width <= READING_WIDTH_MAX) {
        customReadingWidth = width;
        applyReadingWidth();
    }
});
els.optReadingCustom.addEventListener('change', () => {
    els.optReadingCustom.value = String(customReadingWidth);
});


// ---- File open / drop / paste / URL ----
async function stlFileToAscii(buffer) {
    const view = new DataView(buffer);
    // Binary headers can start with "solid" too. The count and record size
    // distinguish those files from ASCII without decoding the binary payload.
    const count = buffer.byteLength >= 84 ? view.getUint32(80, true) : 0;
    if (buffer.byteLength >= 84 && 84 + count * 50 === buffer.byteLength) {
        if (!count) throw new Error('The STL file has no triangular facets.');
        const facets = ['solid model\n'];
        let deadline = performance.now() + 50;
        for (let i = 0; i < count; i++) {
            const values = [];
            for (let j = 0; j < 12; j++) {
                const value = view.getFloat32(84 + i * 50 + j * 4, true);
                if (!Number.isFinite(value)) throw new Error('The STL file contains a non-finite normal or vertex.');
                values.push(value);
            }
            facets.push(`  facet normal ${values.slice(0, 3).join(' ')}\n    outer loop\n      vertex ${values.slice(3, 6).join(' ')}\n      vertex ${values.slice(6, 9).join(' ')}\n      vertex ${values.slice(9, 12).join(' ')}\n    endloop\n  endfacet\n`);
            if (performance.now() >= deadline) { await yieldToMain(); deadline = performance.now() + 50; }
        }
        facets.push('endsolid model');
        return facets.join('');
    }
    const text = new TextDecoder().decode(buffer).replace(/^\uFEFF/, '').trim();
    if (!/^solid\b/i.test(text) || !/\bendsolid\b/i.test(text) || text.includes('\0')) {
        throw new Error('Invalid or truncated STL file.');
    }
    return text;
}

let fileImportGeneration = 0;
let fileImportAbort;
function cancelFileImport() {
    fileImportGeneration++;
    fileImportAbort?.abort(); fileImportAbort = null;
    $('modelImportStatus').hidden = true;
    els.previewPane.removeAttribute('aria-busy');
}
$('cancelModelImport').addEventListener('click', () => { cancelFileImport(); setExportEnabled(!!markdown.trim()); });
// `handle` (File System Access) enables in-place saves for Markdown files only;
// HTML, STL, and 3MF sources are converted, so saving never overwrites them.
async function handleFile(file, handle = null) {
    cancelFileImport();
    const generation = ++fileImportGeneration;
    const abort = new AbortController(); fileImportAbort = abort;
    try {
        const stl = /\.stl$/i.test(file.name);
        if (/\.3mf$/i.test(file.name)) {
            $('modelImportStatus').hidden = false;
            els.previewPane.setAttribute('aria-busy', 'true');
            setExportEnabled(false);
            if (file.size > 128 * 1024 * 1024) throw new Error('3MF archive exceeds 128 MiB.');
            const buffer = await file.arrayBuffer();
            if (generation !== fileImportGeneration) return;
            const result = await runModelJob({ buffer }, abort.signal);
            if (generation !== fileImportGeneration) return;
            Object.defineProperty(result.mesh, '__buffers', { value: { positions: result.positions, colors: result.colors } });
            meshParseCache = { source: result.source, mesh: result.mesh };
            filename = file.name.replace(/\.3mf$/i, '');
            frontmatter = '';
            setThreads([]);
            loadDocument(`\`\`\`su-mesh\n${result.source}\n\`\`\`\n`);
            setViewMode('preview');
            if (result.mesh.warnings.length) toast(result.mesh.warnings.join(' '));
            return;
        }
        const raw = stl ? await stlFileToAscii(await file.arrayBuffer()) : await file.text();
        if (generation !== fileImportGeneration) return;
        filename = file.name.replace(/\.(?:html?|stl)$/i, '').replace(MD_FILE_EXT, '');
        if (stl) {
            let fenceLength = 3;
            for (const match of raw.matchAll(/`+/g)) fenceLength = Math.max(fenceLength, match[0].length + 1);
            const fence = '`'.repeat(fenceLength);
            frontmatter = '';
            setThreads([]);
            loadDocument(`${fence}stl\n${raw}\n${fence}\n`);
        } else {
            const embedded = extractEmbeddedMarkdown(raw);
            const editable = embedded === null && MD_FILE_EXT.test(file.name);
            // MARS: the host files Markdown into the tree, matching a copy already there.
            if (editable && host.openFile) {
                await host.openFile({ name: file.name, text: raw, handle, stamp: file.lastModified });
                setViewMode('preview');
                return;
            }
            loadDocument(parseSource(embedded ?? raw), editable ? handle : null, file.lastModified);
        }
        setViewMode('preview');
    } catch (err) {
        if (generation === fileImportGeneration) toast(`Failed to open file: ${err?.message ?? err}`);
    } finally {
        if (fileImportAbort === abort) { cancelFileImport(); setExportEnabled(!!markdown.trim()); }
    }
}

// Chromium's picker returns a handle for in-place saves; elsewhere use the input.
async function openFile() {
    if (!('showOpenFilePicker' in window)) { els.fileInput.click(); return; }
    try {
        const [handle] = await showOpenFilePicker({
            types: [...MD_PICKER_TYPES, { description: 'HTML, STL, or 3MF', accept: { 'text/html': ['.html', '.htm'], 'model/stl': ['.stl'], 'model/3mf': ['.3mf'] } }],
        });
        handleFile(await handle.getFile(), handle);
    } catch (err) {
        if (err?.name !== 'AbortError') els.fileInput.click();
    }
}
els.openBtn.addEventListener('click', openFile);
els.emptyOpenBtn.addEventListener('click', openFile);
els.fileInput.addEventListener('change', (e) => {
    const file = e.target.files?.[0];
    if (file) handleFile(file);
    els.fileInput.value = '';
});

els.emptyWriteBtn.addEventListener('click', () => setViewMode('edit'));
els.emptyExampleBtn.addEventListener('click', () => {
    cancelFileImport();
    filename = 'Welcome to MARS';
    frontmatter = '';
    setThreads([]);
    loadDocument(EXAMPLE_MD);
});

// If `text` is an HTML document carrying our embedded source
// (<script type="text/markdown">, as produced by the "Embed source" export
// option), pull the original Markdown back out. Returns null otherwise.
function extractEmbeddedMarkdown(text) {
    if (!/<script[^>]*type=["']text\/markdown["']/i.test(text)) return null;
    try {
        const el = new DOMParser().parseFromString(text, 'text/html')
            .querySelector('script[type="text/markdown"]');
        if (!el) return null;
        // Reverse the closing-tag escaping applied by embedSourceTag, and drop
        // the single wrapping newline it adds on each side.
        return el.textContent.replace(/<\\\/(script)/gi, '</$1').replace(/^\n/, '').replace(/\n$/, '');
    } catch { return null; }
}

async function loadFromUrl(url) {
    cancelFileImport();
    // Clear the hydration loading state if a load fails without producing content.
    const fail = (msg) => {
        toast(msg);
        if (!markdown.trim()) { pendingHydration = false; updatePanes(); }
    };
    let target;
    try { target = new URL(url); } catch { fail('Enter a valid URL'); return; }
    if (target.protocol !== 'https:' && target.protocol !== 'http:') { fail('Only http(s) URLs are supported'); return; }
    toast('Fetching Markdown...');
    try {
        const res = await fetch(target.href, { redirect: 'follow' });
        if (!res.ok) { fail(`Fetch failed: HTTP ${res.status}`); return; }
        const text = await res.text();
        const embedded = extractEmbeddedMarkdown(text);
        filename = decodeURIComponent(target.pathname.split('/').pop() || 'document')
            .replace(/\.html?$/i, '').replace(MD_FILE_EXT, '') || 'document';
        loadDocument(parseSource(embedded ?? text));
        setViewMode('preview');
        toast('Loaded from URL');
    } catch {
        fail('Fetch failed (CORS or network). Try a raw, CORS-enabled URL.');
    }
}
els.urlForm.addEventListener('submit', (e) => {
    e.preventDefault();
    const url = els.urlInput.value.trim();
    if (url) loadFromUrl(url);
});

// Drag-and-drop onto the preview or editor.
for (const zone of [els.previewWrap, els.editorMount]) {
    zone.addEventListener('dragover', (e) => {
        if (e.dataTransfer?.types.includes('Files')) { e.preventDefault(); zone.classList.add('dragover'); }
    });
    zone.addEventListener('dragleave', () => zone.classList.remove('dragover'));
    zone.addEventListener('drop', (e) => {
        e.preventDefault();
        zone.classList.remove('dragover');
        const file = e.dataTransfer?.files?.[0];
        if (!file) return;
        // The handle must be requested synchronously, while the drop data is readable.
        const item = [...e.dataTransfer.items].find((i) => i.kind === 'file');
        const pending = item?.getAsFileSystemHandle?.();
        if (!pending) { handleFile(file); return; }
        pending.then((handle) => handleFile(file, handle?.kind === 'file' ? handle : null), () => handleFile(file));
    });
}

// Quick paste: when nothing is loaded and focus isn't in an input/editor,
// a paste drops straight into the preview.
document.addEventListener('paste', (e) => {
    if (markdown.trim() || viewMode !== 'preview') return;
    const t = e.target;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
    const text = e.clipboardData?.getData('text/plain');
    if (!text || !text.trim()) return;
    e.preventDefault();
    if (!host.currentId?.()) filename = 'Pasted';
    loadDocument(parseSource(text));
});

function toast(msg, action) {
    clearTimeout(toastTimer);
    els.toastMsg.textContent = msg;
    els.toastAction.hidden = !action;
    els.toastAction.textContent = action?.label ?? '';
    els.toastAction.onclick = action ? () => { els.toast.style.display = 'none'; action.run(); } : null;
    els.toast.style.display = 'flex';
    toastTimer = setTimeout(() => { els.toast.style.display = 'none'; }, action ? 6000 : 2200);
}

// ---- Keyboard shortcuts ----
document.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === 'Escape') {
        if (!els.blockMenu.hidden) { closeBlockMenu(); return; }
        if (!els.formatBar.hidden) { window.getSelection()?.removeAllRanges(); return; }
        if (activeThread) { activateThread(null); return; }
        if (!els.findBar.hidden) { closeFind(); return; }
        if (viewMode !== 'preview') { setViewMode('preview'); return; }
    }
    const previewKeys = viewMode === 'preview' && !blockEditor && !isEditable(e.target);
    if (previewKeys && e.altKey && !mod && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
        const target = menuTarget ?? hoverTarget;
        if (target && previewInSync()) { e.preventDefault(); closeBlockMenu(); moveTarget(target, e.key === 'ArrowUp' ? -1 : 1); }
        return;
    }
    if (!mod) return;
    if (e.altKey && e.code === 'KeyM') { if (commentableRange()) { e.preventDefault(); startDraft(); } return; }
    const k = e.key.toLowerCase();
    if (k === 'z' && previewKeys) { e.preventDefault(); undoEdit(e.shiftKey); }
    else if ((k === 'b' || k === 'i') && previewKeys && selectionTarget()) { e.preventDefault(); applyFormat(k === 'b' ? 'bold' : 'italic'); }
    else if (k === 'o') { e.preventDefault(); openFile(); }
    else if (k === 'f') { e.preventDefault(); openFind(); }
    else if (k === 'e') { e.preventDefault(); setViewMode(isEdit() ? 'preview' : 'edit'); }
    else if (k === 's') { e.preventDefault(); commitBlockEditor(); saveDocument(); }
});

function saveMarkdown() {
    if (!markdown) return;
    const blob = new Blob([documentText()], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${filename}.md`;
    a.click();
    URL.revokeObjectURL(url);
    toast(`Saved ${filename}.md`);
}

// ---- Persistence: host storage (IndexedDB) + File System Access saves ----
// Every change goes to the host, which keeps the review tree in IndexedDB. Chromium
// can also write back to a linked file; elsewhere Ctrl/Cmd+S downloads a copy.
const MD_PICKER_TYPES = [{ description: 'Markdown', accept: { 'text/markdown': ['.md', '.markdown', '.mdx', '.txt'] } }];
let persistTimer;
let writeQueue = Promise.resolve();
let eol = '\n';
// ponytail: a file with mixed line endings is saved with CRLF throughout.
const withEol = (text) => (eol === '\n' ? text : text.replaceAll('\n', eol));

// Edits work on LF text; CRLF documents get their line endings back on save.
function adoptEol(text) {
    eol = (frontmatter + text).includes('\r\n') ? '\r\n' : '\n';
    frontmatter = frontmatter.replace(/\r\n?/g, '\n');
    return text.replace(/\r\n?/g, '\n');
}

// Starts a new document: resets history and binds (or unbinds) the source file.
// Content opened from the toolbar (file, URL, example) replaces a non-empty document
// with a new tree entry; `fromHost` loads the document the host selected.
function loadDocument(text, handle = null, stamp = 0, fromHost = false) {
    commitBlockEditor();
    if (!fromHost && markdown.trim()) host.detach?.();
    text = adoptEol(text);
    fileHandle = handle;
    fileStamp = stamp;
    fileConflict = false;
    undoStack.length = 0;
    redoStack.length = 0;
    expandedDone.clear();
    draft = null;
    activeThread = null;
    setMarkdown(text);
    savedMarkdown = markdown;
    savedComments = commentsState;
    updateSaveState();
}

const isDirty = () => markdown !== savedMarkdown || commentsState !== savedComments;
const isPersisted = () => filename === persistedName && documentText() === persistedText;
const markPersisted = () => { persistedText = documentText(); persistedName = filename; };
const currentState = () => ({ text: documentText(), name: filename, handle: fileHandle, stamp: fileStamp, dirty: isDirty() });

function updateSaveState() {
    const dirty = isDirty();
    let label = '', title = '';
    if (fileConflict) [label, title] = ['Changed on disk', `${fileHandle.name} changed outside this tab. Press Ctrl/Cmd+S to overwrite it.`];
    else if (fileHandle) [label, title] = dirty ? ['Unsaved to disk', `Saved in this browser. Press Ctrl/Cmd+S to save to ${fileHandle.name}`] : ['Saved', `Changes are saved to ${fileHandle.name} and this browser`];
    else if (markdown.trim() || host.currentId?.()) [label, title] = isPersisted() ? ['Saved', 'Saved in this browser'] : ['Saving…', 'Saving in this browser'];
    els.saveState.textContent = label;
    els.saveState.title = title;
    els.saveState.classList.toggle('su-warn', fileConflict);
}

function persistSoon() {
    updateSaveState();
    clearTimeout(persistTimer);
    persistTimer = setTimeout(persistNow, 500);
}

async function persistNow() {
    clearTimeout(persistTimer);
    if (!isPersisted() && (markdown.trim() || host.currentId?.())) {
        markPersisted();
        try { await host.persist?.(currentState()); } catch (err) { persistedText = persistedName = ''; toast(`Could not save in this browser: ${err?.message ?? err}`); }
        updateSaveState();
    }
    if (fileHandle && !fileConflict && isDirty()
        && (await fileHandle.queryPermission?.({ mode: 'readwrite' })) === 'granted') await writeToHandle();
}

// ---- Host API ----
export { toast };
export const flush = () => persistNow();
export const currentName = () => filename;
export const currentText = () => documentText();
export const currentThreads = () => threads;

export function openDocument({ text = '', name = 'Untitled', handle = null, stamp = 0, dirty = false } = {}) {
    cancelFileImport();
    clearTimeout(persistTimer);
    if (els.findBar && !els.findBar.hidden) closeFind();
    const body = parseSource(text);
    filename = name;
    loadDocument(body, handle, stamp, true);
    if (dirty) savedComments = null;
    markPersisted();
    updateSaveState();
    setDocTitle();
}

// ---- Review changes hooks (MARS only) ----
// `fn` runs after each preview render, before threads anchor to it.
export const onRendered = (fn) => { renderedHooks.push(fn); };

// Shows a search hit once the open document has rendered: `query` opens Find with it, and
// `start` (a body offset) scrolls to the block that contains it.
export function reveal(target) {
    pendingReveal = target;
    if (markdown.trim() && markdown === renderedMarkdown) applyReveal();
}
function applyReveal() {
    const target = pendingReveal;
    pendingReveal = null;
    if (!target || !markdown.trim()) return;
    if (target.query) { els.findInput.value = target.query; openFind(); return; }
    const block = [...els.previewArticle.querySelectorAll(':scope > .su-block')].findLast((b) => Number(b.dataset.start) <= target.start);
    if (!block) return;
    block.scrollIntoView({ block: 'center', behavior: 'smooth' });
    block.classList.add('su-reveal');
    setTimeout(() => block.classList.remove('su-reveal'), 1600);
}
// { fallbackAnchor(thread) -> Range | null, note(thread) -> string | null, isNew(comment) -> boolean }
export const setThreadHooks = (hooks) => { threadHooks = hooks; };
export const refreshThreads = () => { anchorThreads(); renderRail(); };
// The body the preview DOM was rendered from; `.su-block` data-start/data-end index into it.
export const renderedSource = () => renderedMarkdown;
export const inSync = () => markdown === renderedMarkdown;
export const currentAuthor = () => savedAuthor();

// A block's rendered text, and Ranges over offsets into it.
export function blockText(el) {
    const index = textIndex(el);
    const range = (start, end) => {
        const r = document.createRange();
        if (!index.nodes.length) { r.selectNodeContents(el); return r; }
        r.setStart(...textPoint(index, start));
        r.setEnd(...textPoint(index, end, true));
        return r;
    };
    return { text: index.text, range };
}

// Re-quotes threads to new passages: `entries` holds [thread id, preview Range] pairs.
// Quotes are clipped to COMMENT_QUOTE_MAX at a word boundary, so they still match.
export function requote(entries) {
    const index = textIndex();
    let changed = false;
    for (const [id, range] of entries) {
        const t = threads.find((x) => x.id === id);
        if (!t) continue;
        let start = textOffset(index, range.startContainer, range.startOffset);
        let end = textOffset(index, range.endContainer, range.endOffset);
        while (start < end && /\s/.test(index.text[start])) start++;
        while (end > start && /\s/.test(index.text[end - 1])) end--;
        if (end - start > COMMENT_QUOTE_MAX) {
            const cut = index.text.lastIndexOf(' ', start + COMMENT_QUOTE_MAX);
            end = cut > start ? cut : start + COMMENT_QUOTE_MAX;
        }
        if (end <= start) continue;
        Object.assign(t, {
            quote: index.text.slice(start, end),
            prefix: index.text.slice(Math.max(0, start - COMMENT_CONTEXT), start),
            suffix: index.text.slice(end, end + COMMENT_CONTEXT),
        });
        changed = true;
    }
    if (changed) commentsChanged();
}

export function renameDocument(name) {
    filename = name;
    persistedName = name;
    setDocTitle();
}

export function closeDocument() {
    openDocument({ text: '', name: 'Untitled' });
}

// Writes are serialized. Unless `force` is set, a file modified on disk since our
// last read or write is never overwritten.
function writeToHandle(force = false) {
    writeQueue = writeQueue.then(async () => {
        const handle = fileHandle, text = markdown, comments = commentsState, content = documentText();
        if (!handle) return;
        try {
            if (!force && fileStamp && (await handle.getFile()).lastModified !== fileStamp) {
                fileConflict = true;
                toast(`${handle.name} changed on disk. Autosave paused; Ctrl/Cmd+S overwrites it.`);
                return;
            }
            const writable = await handle.createWritable();
            await writable.write(content);
            await writable.close();
            fileStamp = (await handle.getFile()).lastModified;
            savedMarkdown = text;
            savedComments = comments;
            fileConflict = false;
            host.persist?.(currentState());
        } catch (err) {
            toast(`Save failed: ${err?.message ?? err}`);
        } finally {
            updateSaveState();
        }
    });
    return writeQueue;
}

async function saveDocument() {
    if (!markdown.trim()) return;
    if (!fileHandle && 'showSaveFilePicker' in window) {
        try {
            fileHandle = await showSaveFilePicker({ suggestedName: `${filename}.md`, types: MD_PICKER_TYPES });
            fileStamp = 0;
            fileConflict = false;
        } catch (err) {
            if (err?.name === 'AbortError') return;
        }
    }
    if (!fileHandle) {
        saveMarkdown();
        savedMarkdown = markdown;
        savedComments = commentsState;
        updateSaveState();
        return;
    }
    try {
        if ((await fileHandle.requestPermission({ mode: 'readwrite' })) !== 'granted') { toast('Write permission was not granted'); return; }
        const changed = fileStamp && (await fileHandle.getFile()).lastModified !== fileStamp;
        if (changed && !confirm(`${fileHandle.name} changed on disk since it was opened. Overwrite it with this version?`)) return;
    } catch (err) {
        toast(`Save failed: ${err?.message ?? err}`);
        return;
    }
    await writeToHandle(true);
    if (!isDirty()) toast(`Saved ${fileHandle.name}`);
}

export function endLoading(alert = false) {
    pendingHydration = false;
    updatePanes();
    els.shortLinkAlert.hidden = !alert;
}

loadEditor(els.editorMount);
