// Review changes: what changed, marked in the preview, against one of two baselines:
// - `review`: the text before an outside change replaced it (app.rebase); "Mark reviewed" drops it.
// - `opened`: the source text as MARS last received it, shown on request ("since opened").
import { marked } from 'marked';
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { commentKey, splitComments, type Comment, type Thread } from '../share/merge.ts';
import { $ } from '../ui/ui.ts';
import { diffBlocks, diffWords } from './diff.ts';
import * as viewer from './viewer.js';

const FRONTMATTER_RE = /^---\r?\n[\s\S]*?\r?\n---\r?\n/;
// Rendered widgets whose text doesn't line up with their source get the block bar only.
const RICH = 'pre, figure, .su-diagram, .katex';
const MARKS = ['su-diff-add', 'su-diff-edit', 'su-diff-del', 'su-diff-del-end', 'su-diff-focus'];
const KIND = { added: 'Added', edited: 'Edited', removed: 'Removed' } as const;

type Mode = 'review' | 'opened';
interface Base { id: string; mode: Mode; raws: string[]; texts: Map<number, string>; comments: Set<string> }
// `span`: from the first to the last inserted word of an edited block, when words were compared.
interface Change { kind: keyof typeof KIND; el: HTMLElement; old: number[]; at: number; span?: [number, number] }

let wanted: string | null = null;
let base: Base | null = null;
// The document whose "since opened" view the reader asked for.
let sinceOpened: string | null = null;
let changes: Change[] = [];
let focused = -1;
let newCount = 0;
let byOld = new Map<number, Change>();
// Threads whose quoted text was rewritten (the edited block's change) or removed (null).
const followed = new Map<string, Change | null>();

const highlight = 'highlights' in CSS ? new Highlight() : null;
if (highlight) CSS.highlights.set('su-diff-ins', highlight);

const bar = $('changesBar'), label = $('changesLabel'), before = $('diffBefore');
const prevBtn = $<HTMLButtonElement>('changesPrev'), nextBtn = $<HTMLButtonElement>('changesNext'), beforeBtn = $<HTMLButtonElement>('changesBefore');
const doneBtn = $('changesDone'), closeBtn = $('changesClose'), swapBtn = $('changesSwap'), openedBtn = $<HTMLButtonElement>('optSinceOpened');

function parseBase(id: string, mode: Mode, text: string): Base {
    const { body, threads } = splitComments(text.replace(/\r\n?/g, '\n'));
    const raws = marked.lexer(body.replace(FRONTMATTER_RE, '')).filter((t) => t.type !== 'space').map((t) => t.raw);
    return { id, mode, raws, texts: new Map(), comments: new Set(threads.flatMap((t) => t.comments.map(commentKey))) };
}

const oldHtml = (i: number) => marked.parse(base!.raws[i], { async: false });
function oldText(i: number) {
    let text = base!.texts.get(i);
    if (text === undefined) {
        const div = document.createElement('div');
        div.innerHTML = oldHtml(i);
        base!.texts.set(i, text = div.textContent ?? '');
    }
    return text;
}

const author = (name?: string) => name?.trim() || 'Anonymous';
const isNew = (c: Comment) => !!base && !base.comments.has(commentKey(c)) && author(c.author) !== author(viewer.currentAuthor());

function clearMarks() {
    for (const el of document.querySelectorAll<HTMLElement>(MARKS.map((c) => `#previewArticle .${c}`).join(','))) el.classList.remove(...MARKS);
    highlight?.clear();
    changes = [];
    byOld = new Map();
    followed.clear();
    newCount = 0;
}

function markWords(change: Change, previous: string) {
    if (!highlight || change.el.querySelector(RICH)) return;
    const { text, range } = viewer.blockText(change.el);
    const words = diffWords(previous, text) ?? [];
    for (const [start, end] of words) highlight.add(range(start, end));
    if (words.length) change.span = [words[0][0], words.at(-1)![1]];
}

// Where a followed thread attaches: the inserted words of its block, or the whole block.
function followRange(change: Change) {
    const { text, range } = viewer.blockText(change.el);
    return change.span ? range(...change.span) : range(0, text.length);
}

// Recomputes the marks for the rendered preview. After a render with nothing left to show,
// the baseline clears itself.
function apply(fromRender = false) {
    clearMarks();
    if (!base || base.id !== app.state.currentId || !viewer.inSync()) { renderBar(); return; }
    const blocks = [...$('previewArticle').querySelectorAll<HTMLElement>(':scope > .su-block')];
    const src = viewer.renderedSource();
    if (blocks.length || !src.trim()) {
        const ops = diffBlocks(base.raws, blocks.map((b) => src.slice(Number(b.dataset.start), Number(b.dataset.end))));
        for (const op of ops) {
            if (op.kind === 'removed') {
                const last = changes.at(-1);
                if (last?.kind === 'removed' && last.at === op.cur) { last.old.push(op.old); byOld.set(op.old, last); continue; }
                const el = blocks[op.cur] ?? blocks.at(-1);
                if (!el) continue;
                el.classList.add(op.cur < blocks.length ? 'su-diff-del' : 'su-diff-del-end');
                const change: Change = { kind: 'removed', el, old: [op.old], at: op.cur };
                changes.push(change);
                byOld.set(op.old, change);
                continue;
            }
            const el = blocks[op.cur];
            el.classList.add(op.kind === 'added' ? 'su-diff-add' : 'su-diff-edit');
            const change: Change = { kind: op.kind, el, old: op.kind === 'edited' ? [op.old] : [], at: op.cur };
            changes.push(change);
            if (op.kind === 'edited') { byOld.set(op.old, change); markWords(change, oldText(op.old)); }
        }
    }
    newCount = viewer.currentThreads().reduce((n, t) => n + t.comments.filter(isNew).length, 0);
    if (fromRender && base.mode === 'review' && !changes.length && !newCount) {
        const id = base.id;
        base = null;
        app.markReviewed(id);
    }
    if (focused >= changes.length) focused = changes.length - 1;
    changes[focused]?.el.classList.add('su-diff-focus');
    renderBar();
}

// A thread whose quote is gone follows it into the block that replaced the quoted text.
function fallbackAnchor(t: Thread): Range | null {
    if (!base || base.id !== app.state.currentId) return null;
    let best = -1, bestScore = -1;
    base.raws.forEach((_, i) => {
        const text = oldText(i);
        if (!text.includes(t.quote)) return;
        const score = (text.includes((t.prefix ?? '') + t.quote) ? 1 : 0) + (text.includes(t.quote + (t.suffix ?? '')) ? 1 : 0);
        if (score > bestScore) [best, bestScore] = [i, score];
    });
    const change = best < 0 ? undefined : byOld.get(best);
    if (!change) return null;
    if (change.kind === 'removed') { followed.set(t.id, null); return null; }
    followed.set(t.id, change);
    return followRange(change);
}

const since = () => (base?.mode === 'opened' ? 'since opened' : 'since your review');

// The card's quote still shows the old text, so the note only says what happened to it.
function note(t: Thread) {
    if (!base || !followed.has(t.id)) return null;
    return `Quoted text ${followed.get(t.id) ? 'changed' : 'removed'} ${since()}`;
}

function renderBar() {
    const cur = app.current();
    openedBtn.disabled = !cur?.opened;
    openedBtn.textContent = sinceOpened && sinceOpened === cur?.id ? 'Hide changes since opened' : 'Show changes since opened';
    const shown = !!base && base.id === app.state.currentId;
    bar.hidden = !shown;
    if (!shown) { hideBefore(); return; }
    const c = changes[focused];
    const count = `${changes.length} change${changes.length === 1 ? '' : 's'}`;
    const replies = newCount ? ` · ${newCount} new repl${newCount === 1 ? 'y' : 'ies'}` : '';
    label.textContent = c ? `${KIND[c.kind]} · ${focused + 1} of ${changes.length}${replies}` : `${changes.length ? count : 'No text changes'} ${since()}${replies}`;
    prevBtn.disabled = nextBtn.disabled = !changes.length;
    beforeBtn.hidden = !c || c.kind === 'added';
    doneBtn.hidden = base!.mode !== 'review';
    closeBtn.hidden = base!.mode !== 'opened';
    swapBtn.hidden = !(cur?.pending && cur.opened);
    swapBtn.title = base!.mode === 'review' ? 'Compare with the text as opened' : 'Compare with your last review';
    swapBtn.setAttribute('aria-label', swapBtn.title);
}

// Shows or hides "changes since opened" for the open document.
function setSinceOpened(on: boolean) {
    sinceOpened = on ? app.state.currentId : null;
    load();
}

function go(step: number) {
    if (!changes.length) return;
    hideBefore();
    changes[focused]?.el.classList.remove('su-diff-focus');
    focused = focused < 0 ? (step > 0 ? 0 : changes.length - 1) : (focused + step + changes.length) % changes.length;
    const { el } = changes[focused];
    el.classList.add('su-diff-focus');
    el.scrollIntoView({ block: 'center', behavior: 'smooth' });
    renderBar();
}

function hideBefore() {
    if (before.matches(':popover-open')) before.hidePopover();
    beforeBtn.setAttribute('aria-expanded', 'false');
}

function showBefore() {
    const c = changes[focused];
    if (!c || !base) return;
    if (before.matches(':popover-open')) { hideBefore(); return; }
    before.innerHTML = `<div class="diff-before-head">${base.mode === 'opened' ? 'As opened' : 'Before your last review'}</div><div class="preview-article">${c.old.map(oldHtml).join('')}</div>`;
    before.showPopover();
    const r = c.el.getBoundingClientRect(), { width, height } = before.getBoundingClientRect();
    before.style.left = `${Math.round(Math.min(Math.max(8, r.left), innerWidth - width - 8))}px`;
    before.style.top = `${Math.round(r.bottom + 6 + height > innerHeight - 8 ? Math.max(8, r.top - height - 6) : r.bottom + 6)}px`;
    beforeBtn.setAttribute('aria-expanded', 'true');
}

async function markReviewed() {
    const id = app.state.currentId;
    if (!id || base?.mode !== 'review') return;
    const requotes = [...followed].filter(([, c]) => c?.el.isConnected).map(([tid, c]): [string, Range] => [tid, followRange(c!)]);
    if (requotes.length) viewer.requote(requotes);
    await app.markReviewed(id);
    viewer.toast('Marked as reviewed');
}

prevBtn.addEventListener('click', () => go(-1));
nextBtn.addEventListener('click', () => go(1));
beforeBtn.addEventListener('click', showBefore);
doneBtn.addEventListener('click', markReviewed);
closeBtn.addEventListener('click', () => setSinceOpened(false));
swapBtn.addEventListener('click', () => setSinceOpened(base?.mode !== 'opened'));
openedBtn.addEventListener('click', () => {
    $('optionsPopover').hidePopover();
    setSinceOpened(sinceOpened !== app.state.currentId);
});
before.addEventListener('toggle', (e) => { if ((e as ToggleEvent).newState === 'closed') beforeBtn.setAttribute('aria-expanded', 'false'); });

viewer.onRendered(() => apply(true));
viewer.setThreadHooks({ fallbackAnchor, note, isNew });

// Loads the baseline the open document calls for: "since opened" when asked for, otherwise
// the pending review.
async function load() {
    const cur = app.current();
    if (sinceOpened !== cur?.id) sinceOpened = null;
    const mode: Mode | null = cur && sinceOpened === cur.id && cur.opened ? 'opened' : cur?.pending ? 'review' : null;
    const want = mode ? `${mode}:${cur!.id}` : null;
    if (want === wanted) { renderBar(); return; }
    wanted = want;
    base = null;
    focused = -1;
    if (mode) {
        const text = await db.getSide(mode === 'opened' ? 'opened' : 'base', cur!.id);
        if (wanted !== want) return;
        if (text === undefined) { if (mode === 'review') app.markReviewed(cur!.id); renderBar(); return; }
        base = parseBase(cur!.id, mode, text);
    }
    apply();
    viewer.refreshThreads();
}
app.onChange(load);
