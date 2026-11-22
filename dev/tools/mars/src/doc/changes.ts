// Review changes: what changed, marked in the preview, against one of two baselines:
// - `review`: the text before an outside change replaced it (app.rebase); "Mark reviewed" drops it.
// - `opened`: the source text as MARS last received it, shown on request ("since opened").
//
// This module owns the changes bar, the block and word marks in the preview, and the "before"
// popover. Its thread hooks keep a comment attached after its quote changed. It reads the baselines
// and never writes them: `app.rebase` stores the review baseline and `app.markReviewed` deletes it.
// A baseline is document text from another person. Its HTML reaches the page only through the
// preview HTML policy (see `baselineRenderer`). Marks are valid only while the preview is in sync
// with the source (`viewer.inSync`).
import { marked } from 'marked';
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { commentKey, splitComments, type Comment, type Thread } from '../share/merge.ts';
import { hideTooltip } from '../ui/feedback.ts';
import { icon } from '../ui/icons.ts';
import { $ } from '../ui/ui.ts';
import { diffBlocks, diffWords } from './diff.ts';
import * as viewer from './viewer.js';
import { tableReviewBlocks } from './table.ts';
import { applyTableLayouts, tableMetadataHtml } from './table-layout.ts';
import { safeRawHtml, safeRenderedHtml } from './preview-safety.ts';
import { openSourceReview } from './review-workspace.ts';

/** Leading YAML frontmatter. The preview does not render it, so the baseline drops it too. */
const FRONTMATTER_RE = /^---\r?\n[\s\S]*?\r?\n---\r?\n/;
// Rendered widgets whose text doesn't line up with their source get the block bar only.
const RICH = 'pre, figure, .su-diagram, .katex';
/** Every class that this module adds to a preview block. `clearMarks` removes them all. */
const MARKS = ['su-diff-add', 'su-diff-edit', 'su-diff-del', 'su-diff-del-end', 'su-diff-focus'];
/** Label of each change kind in the changes bar. */
const KIND = { added: 'Added', edited: 'Edited', removed: 'Removed' } as const;

type Mode = 'review' | 'opened';
/**
 * A parsed baseline of one document. `raws` holds its blocks as source text, and `texts` caches the
 * rendered text of a block by index. `comments` holds the keys of the comments it contained.
 */
interface Base {
    id: string;
    mode: Mode;
    raws: string[];
    texts: Map<number, string>;
    comments: Set<string>;
    source: string;
}
/**
 * One marked change in the preview. `el` is the block that carries the mark, `old` lists the
 * baseline blocks behind it, and `at` is its current block index. A run of removed blocks shares
 * one change on the block that follows the run.
 * `span`: from the first to the last inserted word of an edited block, when words were compared.
 */
interface Change {
    kind: keyof typeof KIND;
    el: HTMLElement;
    old: number[];
    at: number;
    span?: [number, number];
}

// `<mode>:<id>` of the baseline that `load` last asked for. A slower read for another key is dropped.
let wanted: string | null = null;
let base: Base | null = null;
// The document whose "since opened" view the reader asked for.
let sinceOpened: string | null = null;
let changes: Change[] = [];
// Index into `changes` of the change that Previous and Next selected, or -1.
let focused = -1;
let beforeTrigger: HTMLButtonElement | undefined;
// Comments that are absent from the baseline and written by another author.
let newCount = 0;
// Baseline block index to the change that replaced or removed that block.
let byOld = new Map<number, Change>();
// Threads whose quoted text was rewritten (the edited block's change) or removed (null).
const followed = new Map<string, Change | null>();

// Inserted words use the CSS Custom Highlight API, so marking them changes no preview DOM.
// Without the API, an edited block shows its block bar only.
const highlight = 'highlights' in CSS ? new Highlight() : null;
if (highlight) CSS.highlights.set('su-diff-ins', highlight);

const bar = $('changesBar'),
    label = $('changesLabel'),
    before = $('diffBefore');
const prevBtn = $<HTMLButtonElement>('changesPrev'),
    nextBtn = $<HTMLButtonElement>('changesNext');
const doneBtn = $('changesDone'),
    closeBtn = $('changesClose'),
    swapBtn = $('changesSwap'),
    openedBtn = $<HTMLButtonElement>('optSinceOpened');

/** Splits baseline text into the same blocks as the live preview: no comments block, no frontmatter. */
function parseBase(id: string, mode: Mode, text: string): Base {
    const { body, threads } = splitComments(text.replace(/\r\n?/g, '\n'));
    const raws = tableReviewBlocks(body.replace(FRONTMATTER_RE, ''));
    return {
        id,
        mode,
        raws,
        texts: new Map(),
        comments: new Set(threads.flatMap((t) => t.comments.map(commentKey))),
        source: text,
    };
}

// The baseline is document text from another person, like the preview. Render it with the same
// HTML policy: sanitized raw HTML, escaped raw-block text, and a final pass over the output.
const baselineRenderer = new marked.Renderer();
baselineRenderer.html = ({ text }) => tableMetadataHtml(text) ?? safeRawHtml(text, true);
const renderBaselineText = baselineRenderer.text;
baselineRenderer.text = function (token) {
    return 'escaped' in token && token.escaped && !('tokens' in token && token.tokens)
        ? token.text.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`)
        : renderBaselineText.call(this, token);
};
/** Sanitized HTML of baseline block `i`. */
const oldHtml = (i: number) =>
    safeRenderedHtml(marked.parse(base!.raws[i], { async: false, renderer: baselineRenderer }));
/** Rendered text of baseline block `i`, parsed in a detached element and cached on the baseline. */
function oldText(i: number) {
    let text = base!.texts.get(i);
    if (text === undefined) {
        const div = document.createElement('div');
        div.innerHTML = oldHtml(i);
        base!.texts.set(i, (text = div.textContent ?? ''));
    }
    return text;
}

const author = (name?: string) => name?.trim() || 'Anonymous';
/** True for a comment that the baseline lacks and that another author wrote. */
const isNew = (c: Comment) =>
    !!base && !base.comments.has(commentKey(c)) && author(c.author) !== author(viewer.currentAuthor());

function clearMarks() {
    hideBefore();
    for (const button of $('previewArticle').querySelectorAll('.su-diff-before')) button.remove();
    for (const el of document.querySelectorAll<HTMLElement>(MARKS.map((c) => `#previewArticle .${c}`).join(',')))
        el.classList.remove(...MARKS);
    highlight?.clear();
    changes = [];
    byOld = new Map();
    followed.clear();
    newCount = 0;
}

/** Highlights the words that an edited block gained since `previous`, and records their span. */
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

/**
 * Recomputes the marks for the rendered preview. After a render with nothing left to show,
 * the baseline clears itself.
 *
 * @param fromRender True when a preview render triggered the call. Only then can an empty
 *   `review` result mark the document as reviewed.
 */
function apply(fromRender = false) {
    clearMarks();
    // Block offsets belong to the rendered source. Skip the diff until the preview catches up.
    if (!base || base.id !== app.state.currentId || !viewer.inSync()) {
        renderBar();
        return;
    }
    const blocks = [...$('previewArticle').querySelectorAll<HTMLElement>(':scope > .su-block')];
    const src = viewer.renderedSource();
    // A preview without source-mapped blocks (a very large document) has no ranges to compare.
    if (blocks.length || !src.trim()) {
        const ops = diffBlocks(
            base.raws,
            blocks.map((b) => src.slice(Number(b.dataset.start), Number(b.dataset.end))),
        );
        for (const op of ops) {
            if (op.kind === 'removed') {
                // Consecutive removed blocks share one mark on the block that follows them.
                const last = changes.at(-1);
                if (last?.kind === 'removed' && last.at === op.cur) {
                    last.old.push(op.old);
                    byOld.set(op.old, last);
                    continue;
                }
                // A block removed from the end has no successor. Mark the end of the last block.
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
            if (op.kind === 'edited') {
                byOld.set(op.old, change);
                markWords(change, oldText(op.old));
            }
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
    const previous = new Map<HTMLElement, number[]>();
    for (const { el, old } of changes) {
        if (!old.length) continue;
        const blocks = previous.get(el);
        if (blocks) blocks.push(...old);
        else previous.set(el, [...old]);
    }
    for (const [el, old] of previous) {
        const trigger = document.createElement('button');
        trigger.type = 'button';
        trigger.className = 'icon-btn su-diff-before';
        trigger.dataset.tooltip = 'Show before';
        trigger.setAttribute('aria-label', 'Show before');
        trigger.setAttribute('aria-controls', before.id);
        trigger.setAttribute('aria-haspopup', 'dialog');
        trigger.setAttribute('aria-expanded', 'false');
        trigger.innerHTML = icon('eye');
        trigger.addEventListener('click', () => showBefore(trigger, old));
        el.append(trigger);
    }
    renderBar();
}

/**
 * A thread whose quote is gone follows it into the block that replaced the quoted text.
 * The baseline block that contains the quote, preferably with its prefix and suffix, selects the
 * change. Returns null when no baseline block holds the quote, or when that block is unchanged
 * or removed.
 */
function fallbackAnchor(t: Thread): Range | null {
    if (!base || base.id !== app.state.currentId) return null;
    let best = -1,
        bestScore = -1;
    base.raws.forEach((_, i) => {
        const text = oldText(i);
        if (!text.includes(t.quote)) return;
        const score =
            (text.includes((t.prefix ?? '') + t.quote) ? 1 : 0) + (text.includes(t.quote + (t.suffix ?? '')) ? 1 : 0);
        if (score > bestScore) [best, bestScore] = [i, score];
    });
    const change = best < 0 ? undefined : byOld.get(best);
    if (!change) return null;
    if (change.kind === 'removed') {
        followed.set(t.id, null);
        return null;
    }
    followed.set(t.id, change);
    return followRange(change);
}

const since = () => (base?.mode === 'opened' ? 'since opened' : 'since your review');

// The card's quote still shows the old text, so the note only says what happened to it.
function note(t: Thread) {
    if (!base || !followed.has(t.id)) return null;
    return `Quoted text ${followed.get(t.id) ? 'changed' : 'removed'} ${since()}`;
}

/** Updates the changes bar and the "since opened" menu item for the open document. */
function renderBar() {
    const cur = app.current();
    openedBtn.disabled = !cur?.opened;
    openedBtn.textContent =
        sinceOpened && sinceOpened === cur?.id ? 'Hide changes since opened' : 'Show changes since opened';
    const shown = !!base && base.id === app.state.currentId;
    bar.hidden = !shown;
    if (!shown) {
        hideBefore();
        return;
    }
    const c = changes[focused];
    const opened = base!.mode === 'opened';
    const count = `${changes.length}${opened ? '' : ' new'} change${changes.length === 1 ? '' : 's'}`;
    const replies = newCount ? ` · ${newCount} new repl${newCount === 1 ? 'y' : 'ies'}` : '';
    label.textContent = c
        ? `${KIND[c.kind]} · ${focused + 1} of ${changes.length}${replies}`
        : `${changes.length ? count : 'No text changes'}${opened ? ' since opened' : ''}${replies}`;
    prevBtn.disabled = nextBtn.disabled = !changes.length;
    doneBtn.hidden = base!.mode !== 'review';
    closeBtn.hidden = base!.mode !== 'opened';
    swapBtn.hidden = !(cur?.pending && cur.opened);
    const swapLabel = opened ? 'Compare with your last review' : 'Compare with the text as opened';
    swapBtn.dataset.tooltip = swapLabel;
    swapBtn.setAttribute('aria-label', swapLabel);
}

// Shows or hides "changes since opened" for the open document.
function setSinceOpened(on: boolean) {
    sinceOpened = on ? app.state.currentId : null;
    load();
}

/** Moves the focus to the next (`1`) or previous (`-1`) change, wraps around, and scrolls to it. */
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
    beforeTrigger?.setAttribute('aria-expanded', 'false');
    beforeTrigger = undefined;
}

/** Show the baseline next to the block that opened it, independent of review navigation. */
function showBefore(trigger: HTMLButtonElement, old: number[]) {
    if (!base || !viewer.inSync()) return;
    hideTooltip();
    const closing = before.matches(':popover-open') && beforeTrigger === trigger;
    hideBefore();
    if (closing) return;
    $('diffBeforeTitle').textContent = base.mode === 'opened' ? 'As opened' : 'Before your last review';
    $('diffBeforeBody').innerHTML = old.map(oldHtml).join('');
    applyTableLayouts(before, true);
    beforeTrigger = trigger;
    before.showPopover({ source: trigger });
    const r = trigger.getBoundingClientRect(),
        { width, height } = before.getBoundingClientRect();
    before.style.left = `${Math.round(Math.max(8, Math.min(r.right - width, innerWidth - width - 8)))}px`;
    before.style.top = `${Math.round(r.bottom + 6 + height > innerHeight - 8 ? Math.max(8, r.top - height - 6) : r.bottom + 6)}px`;
    trigger.setAttribute('aria-expanded', 'true');
    before.querySelector('button')?.focus({ preventScroll: true });
}

/**
 * Ends the pending review of the open document. A thread that followed its quote into an edited
 * block is quoted again from that block first, so it stays attached without the baseline.
 */
async function markReviewed() {
    const id = app.state.currentId;
    if (!id || base?.mode !== 'review') return;
    hideTooltip();
    const requotes = [...followed]
        .filter(([, c]) => c?.el.isConnected)
        .map(([tid, c]): [string, Range] => [tid, followRange(c!)]);
    if (requotes.length) viewer.requote(requotes);
    await app.markReviewed(id);
    viewer.toast('Marked as reviewed');
}

prevBtn.addEventListener('click', () => go(-1));
nextBtn.addEventListener('click', () => go(1));
$('changesCompare').addEventListener('click', () => {
    if (base && base.id === app.state.currentId) {
        hideTooltip();
        openSourceReview(
            app.current()?.name ?? 'Document',
            base.mode === 'opened' ? 'As opened' : 'Before your last review',
            base.source,
            viewer.currentText(),
        );
    }
});
doneBtn.addEventListener('click', markReviewed);
closeBtn.addEventListener('click', () => setSinceOpened(false));
swapBtn.addEventListener('click', () => setSinceOpened(base?.mode !== 'opened'));
openedBtn.addEventListener('click', () => {
    $('optionsPopover').hidePopover();
    setSinceOpened(sinceOpened !== app.state.currentId);
});
before.addEventListener('toggle', (e) => {
    if ((e as ToggleEvent).newState === 'closed' && !before.matches(':popover-open')) hideBefore();
});

viewer.onRendered(() => apply(true));
viewer.setThreadHooks({ fallbackAnchor, note, isNew });

/**
 * Loads the baseline the open document calls for: "since opened" when asked for, otherwise
 * the pending review. Runs on every app change and reads storage only when the wanted
 * baseline changed. A `review` document without a stored baseline is marked as reviewed.
 */
async function load() {
    const cur = app.current();
    if (sinceOpened !== cur?.id) sinceOpened = null;
    const mode: Mode | null = cur && sinceOpened === cur.id && cur.opened ? 'opened' : cur?.pending ? 'review' : null;
    const want = mode ? `${mode}:${cur!.id}` : null;
    if (want === wanted) {
        renderBar();
        return;
    }
    wanted = want;
    base = null;
    focused = -1;
    if (mode) {
        const text = await db.getSide(mode === 'opened' ? 'opened' : 'base', cur!.id);
        // Another document or mode was requested during the read. That call owns the state.
        if (wanted !== want) return;
        if (text === undefined) {
            if (mode === 'review') app.markReviewed(cur!.id);
            renderBar();
            return;
        }
        base = parseBase(cur!.id, mode, text);
    }
    apply();
    viewer.refreshThreads();
}
app.onChange(load);
