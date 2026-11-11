// Sidebar search: the filter matches document names and text, and, when smart search is on,
// lists passages that match by meaning. Also the smart-search settings and the suggestion
// to turn it on as the sidebar grows.
//
// This module owns the search state behind the sidebar list and the smart-search controls. The
// sidebar renders the results (`sidebar.ts`), and `embed.ts` owns the model and the index.
// - `textMatches` and `smartMatches` are synchronous and safe to call on every render. They
//   start background work and call the `onUpdate` listeners when a newer answer exists.
// - Snippets are plain text from documents. The caller escapes them.
// - Dialog and badge markup here uses fixed strings and model table values only.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { icon } from '../ui/icons.ts';
import { $, esc, modal, openMenu } from '../ui/ui.ts';
import * as viewer from '../doc/viewer.js';
import { type Chunk, nudgeDue, searchBody, snippet, threadChunks } from './chunks.ts';
import {
    activeDevice,
    disable,
    enable,
    enabledModel,
    indexRevision,
    isDownloaded,
    isPaused,
    MODELS,
    modelUsage,
    onIndexed,
    onStatus,
    pause,
    placementOf,
    reindex,
    removeModel,
    resume,
    search,
    type Hit,
    type ModelKey,
} from './embed.ts';

const smartBtn = $('sbSmartBtn'),
    statusEl = $('sbSmartStatus');
const listeners = new Set<() => void>();
/** Registers a listener that runs when search results or smart-search settings changed. */
export const onUpdate = (fn: () => void) => {
    listeners.add(fn);
};
const notify = () => {
    for (const fn of listeners) fn();
};

// ---- Text matches ----
// Searchable text of each document in memory. An entry is current when its `at` equals the
// `updatedAt` of the document.
const texts = new Map<string, { at: number; body: string; threads: Chunk[] }>();
let loadingTexts: Promise<void> | null = null;

/** Reads the text of each document whose entry is missing or behind, one load at a time, then notifies. */
function refreshTexts() {
    loadingTexts ??= (async () => {
        const docs = app.state.nodes.filter((n) => n.kind === 'doc');
        for (const n of docs) {
            if (texts.get(n.id)?.at === n.updatedAt) continue;
            const at = n.updatedAt; // Before the read, so a save during it refreshes the entry later.
            const text = (await db.getText(n.id)) ?? '',
                body = searchBody(text);
            texts.set(n.id, { at, body, threads: threadChunks(text, body) });
        }
        for (const id of texts.keys()) if (!app.node(id)) texts.delete(id);
    })().finally(() => {
        loadingTexts = null;
        notify();
    });
}

/**
 * Documents whose text or review comments contain `query`, with a snippet around the first
 * match; a match in a comment carries the commented passage's offset. Loads the texts on
 * first use and re-renders the sidebar when they arrive.
 *
 * A query under 2 characters matches nothing. The match ignores case. Results come from the
 * cached texts, so a document that changed can show its previous match until the reload finishes.
 */
export function textMatches(query: string) {
    const out = new Map<string, { snippet: string; start?: number }>();
    if (query.length < 2) return out;
    let stale = false;
    for (const n of app.state.nodes) {
        if (n.kind !== 'doc') continue;
        const t = texts.get(n.id);
        if (t?.at !== n.updatedAt) stale = true;
        if (!t) continue;
        const s = snippet(t.body, query);
        if (s) {
            out.set(n.id, { snippet: s });
            continue;
        }
        for (const c of t.threads) {
            const inComment = snippet(c.text, query);
            if (inComment) {
                out.set(n.id, { snippet: inComment, start: c.start });
                break;
            }
        }
    }
    if (stale && !loadingTexts) refreshTexts();
    return out;
}

// ---- Smart matches ----
/**
 * The last query's passages by meaning, against index revision `at`. `pending` while the
 * query is being embedded.
 */
type Smart = { query: string; at: number; hits: Hit[]; pending: boolean; error?: string };
let smart: Smart = { query: '', at: -1, hits: [], pending: false };
let smartTimer: ReturnType<typeof setTimeout> | undefined;
const same = (s: Smart, query: string, at: number) => s.query === query && s.at === at;

/**
 * Passages that match `query` by meaning, for the sidebar's ranked list. Starts a search
 * (debounced) when the query or the index changed, and re-renders the sidebar when it finishes.
 * After an index change, the previous matches stay on screen, without the pending note, until the new ones arrive.
 *
 * Returns null when smart search is off or the query has under 3 characters. The search starts
 * 350 ms after the last call with a new query or index revision. An answer for an earlier query
 * or revision is dropped.
 */
export function smartMatches(query: string) {
    if (!enabledModel() || query.length < 3) return null;
    const at = indexRevision();
    if (!same(smart, query, at)) {
        const requery = smart.query === query && !smart.pending;
        smart = { query, at, hits: requery ? smart.hits : [], pending: !requery };
        clearTimeout(smartTimer);
        smartTimer = setTimeout(async () => {
            try {
                const hits = await search(query);
                if (same(smart, query, at)) smart = { query, at, hits, pending: false };
            } catch (err) {
                if (same(smart, query, at))
                    smart = { query, at, hits: [], pending: false, error: (err as Error)?.message ?? String(err) };
            }
            if (same(smart, query, at)) notify();
        }, 350);
    }
    return smart;
}
onIndexed(notify);

// ---- Settings ----
/** 'GPU' or 'CPU' for the device of the loaded model, or undefined before a model loaded. */
const deviceLabel = () => (({ webgpu: 'GPU', wasm: 'CPU' }) as Record<string, string>)[activeDevice() ?? ''];

/** Updates the pressed state and tooltip of the sparkles button. */
function syncButton() {
    const key = enabledModel(),
        on = deviceLabel();
    smartBtn.setAttribute('aria-pressed', String(!!key));
    smartBtn.title = key
        ? `Smart search: on (${MODELS[key].label} model${on ? `, ${on}` : ''})`
        : 'Smart search: find passages by meaning (off)';
}

// The badge beside the Documents title. Screen readers hear state changes only, not every
// "Indexing N of M".
const liveEl = $('sbSmartLive');
let lastState: string | null = null;
onStatus((s) => {
    statusEl.hidden = !s;
    if (s) {
        const glyph =
            s.state === 'paused'
                ? icon('pause', 12)
                : s.state === 'error'
                  ? icon('warning', 12)
                  : '<span class="sb-spin" aria-hidden="true"></span>';
        statusEl.innerHTML = `${glyph}<span>${esc(s.label)}</span>`;
        statusEl.dataset.state = s.state;
        statusEl.title = `${s.text}. Click for smart-search options.`;
    }
    const state = s?.state ?? null;
    if (state !== lastState)
        liveEl.textContent = s ? s.text : lastState === 'index' ? 'Smart-search index is up to date' : '';
    lastState = state;
    syncButton();
});
statusEl.addEventListener('click', () => (enabledModel() ? smartMenu(statusEl) : turnOn()));

/** One-line description of each model in the picker. */
const OPTION_TEXT: Record<ModelKey, string> = {
    fast: 'Small download, quick to index. English only.',
    best: 'Closest matches, in any language. Needs a graphics card.',
};

/**
 * The model picker. Resolves with the chosen model, or null when cancelled.
 * A model that cannot run in this browser is disabled. A model that is not downloaded shows the
 * download size of the weights this browser would load.
 */
async function chooseModel(current: ModelKey | null): Promise<ModelKey | null> {
    const keys = Object.keys(MODELS) as ModelKey[];
    const info = Object.fromEntries(
        await Promise.all(
            keys.map(async (k) => [k, { p: await placementOf(k), have: await isDownloaded(k) }] as const),
        ),
    );
    const option = (key: ModelKey) => {
        const m = MODELS[key],
            { p, have } = info[key],
            off = !p;
        const text = off ? "Needs a graphics card (WebGPU), which this browser doesn't offer." : OPTION_TEXT[key];
        return `<label class="model-option${off ? ' disabled' : ''}"><input type="radio" name="smartModel" value="${key}"${key === (current ?? 'fast') && !off ? ' checked' : ''}${off ? ' disabled' : ''}><span><strong>${m.label}</strong>${have ? ' <span class="model-badge">Downloaded</span>' : p ? ` · ${fmtBytes(p.bytes)} download` : ''}<span class="opt-desc">${text}<span class="model-id">${m.name}</span></span></span></label>`;
    };
    const { value } = await modal({
        title: current ? 'Change the search model' : 'Turn on smart search',
        html: `<p>Smart search finds text by what it means, not only by its exact words.</p>
<p>MARS downloads a search model once. After that, indexing and searching happen in this browser, and your documents never leave it.</p>
<fieldset class="model-choice"><legend>Model</legend>${option('fast')}${option('best')}</fieldset>
<p class="su-dialog-fineprint">${current ? 'Switching rebuilds the index with the new model.' : 'Change the model, remove downloaded models, or turn smart search off from the sparkles button in the search box.'}</p>`,
        buttons: [
            { label: 'Cancel', value: '' },
            { label: current ? 'Switch model' : 'Turn on', value: 'ok', primary: true },
        ],
    });
    if (value !== 'ok') return null;
    return (
        ($('marsModalBody').querySelector<HTMLInputElement>('input[name="smartModel"]:checked')?.value as ModelKey) ??
        'fast'
    );
}

/**
 * Opens the model picker and turns smart search on with the chosen model, or switches the model.
 * Does nothing when the reader cancels or keeps the current model. A switch rebuilds the index.
 */
export async function turnOn() {
    const current = enabledModel();
    const key = await chooseModel(current);
    if (!key || key === current) return;
    hideNudge();
    await enable(key);
    syncButton();
    viewer.toast(
        `Smart search is on (${MODELS[key].label} model). ${isPaused() ? 'Indexing stays paused.' : 'Indexing your documents…'}`,
    );
    notify();
}

const fmtBytes = (b: number) => (b < 1e6 ? `${Math.max(1, Math.round(b / 1e3))} KB` : `${Math.round(b / 1e6)} MB`);

smartBtn.addEventListener('click', () => (enabledModel() ? smartMenu(smartBtn) : turnOn()));

/** Opens the smart-search menu: the model, pause or resume, rebuild, model removal, and turn off. */
async function smartMenu(anchor: HTMLElement) {
    const key = enabledModel();
    if (!key) return;
    const usage = await modelUsage();
    const unused = (Object.keys(usage) as ModelKey[]).filter((k) => k !== key);
    openMenu(
        anchor,
        [
            {
                label: `On · ${MODELS[key].label} model`,
                icon: 'sparkles',
                sub: [
                    MODELS[key].langs,
                    deviceLabel() && `runs on the ${deviceLabel()}`,
                    usage[key] && `${fmtBytes(usage[key]!)} downloaded`,
                ]
                    .filter(Boolean)
                    .join(', '),
                disabled: true,
            },
            { label: 'Change model…', icon: 'gear', run: turnOn },
            isPaused()
                ? {
                      label: 'Resume indexing',
                      icon: 'play',
                      sub: 'Indexes what changed while paused',
                      run: () => {
                          resume();
                          viewer.toast('Indexing resumed');
                      },
                  }
                : {
                      label: 'Pause indexing',
                      icon: 'pause',
                      sub: 'In all MARS tabs, until resumed',
                      run: () => {
                          pause();
                          viewer.toast('Indexing paused');
                      },
                  },
            {
                label: 'Rebuild the index',
                icon: 'refresh',
                run: async () => {
                    await enable(key, true);
                    viewer.toast(
                        isPaused()
                            ? 'Index cleared. Resume indexing to rebuild it.'
                            : 'Rebuilding the smart-search index',
                    );
                },
            },
            ...(unused.length
                ? [
                      'separator' as const,
                      ...unused.map((k) => ({
                          label: `Remove the ${MODELS[k].label} model`,
                          icon: 'trash' as const,
                          sub: `Frees ${fmtBytes(usage[k]!)}`,
                          run: async () => {
                              await removeModel(k);
                              viewer.toast(`Removed the ${MODELS[k].label} model`);
                          },
                      })),
                  ]
                : []),
            'separator',
            {
                label: 'Turn off smart search',
                icon: 'cross',
                sub: 'Removes the index and the downloaded models',
                danger: true,
                run: async () => {
                    await disable();
                    syncButton();
                    notify();
                    viewer.toast('Smart search is off');
                },
            },
        ],
        'Smart search',
    );
}

// ---- Suggestion ----
// Shown at 6, 20, and 50 documents, then every 10 more, until turned on or declined for good.
/** `localStorage` key of the suggestion state. */
const NUDGE_KEY = 'mars.smartNudge';
const nudge = $('smartNudge');
/** `shown` is the last threshold that showed the suggestion. `never` stops it for good. */
type NudgeState = { shown: number; never?: boolean };
const readNudge = (): NudgeState => {
    try {
        return JSON.parse(localStorage.getItem(NUDGE_KEY) ?? '') || { shown: 0 };
    } catch {
        return { shown: 0 };
    }
};
const writeNudge = (s: NudgeState) => {
    try {
        localStorage.setItem(NUDGE_KEY, JSON.stringify(s));
    } catch {}
};
const hideNudge = () => {
    nudge.hidden = true;
};

/** Shows the suggestion when the document count reached a threshold that has not shown it yet. */
function checkNudge() {
    const state = readNudge();
    if (enabledModel() || state.never || !nudge.hidden) return;
    const count = app.state.nodes.filter((n) => n.kind === 'doc').length;
    const due = nudgeDue(count, state.shown);
    if (due === null) return;
    writeNudge({ ...state, shown: due });
    nudge.hidden = false;
}

$('smartNudgeClose').addEventListener('click', hideNudge);
$('smartNudgeNever').addEventListener('click', () => {
    writeNudge({ ...readNudge(), never: true });
    hideNudge();
});
$('smartNudgeOn').addEventListener('click', turnOn);

// Check for the suggestion 1.5 seconds after the last tree change.
let nudgeTimer: ReturnType<typeof setTimeout> | undefined;
app.onChange(() => {
    clearTimeout(nudgeTimer);
    nudgeTimer = setTimeout(checkNudge, 1500);
});

syncButton();
// Start one indexing pass 3 seconds after startup.
if (enabledModel()) setTimeout(reindex, 3000);
