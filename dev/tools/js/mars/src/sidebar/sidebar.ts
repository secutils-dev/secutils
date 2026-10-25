// Documents sidebar: a docked, hideable tree of folders and documents (EuiCollapsibleNav +
// EuiTreeView look). Drag rows to move or reorder them; drop files to import them.
import * as app from '../app.ts';
import { ancestors, canMove, childrenOf, textBytes, type TreeNode } from '../tree/model.ts';
import { icon } from '../ui/icons.ts';
import { $, confirmDanger, esc, modal, openMenu, type MenuEntry } from '../ui/ui.ts';
import * as viewer from '../doc/viewer.js';
import { setViewMode, toast } from '../doc/viewer.js';
import { onUpdate, smartMatches, textMatches } from '../search/search.ts';
import * as io from '../io/io.ts';
import * as share from '../share/share.ts';
import * as db from '../store/db.ts';

const tree = $('tree');
const filterInput = $<HTMLInputElement>('sbFilter');
const NARROW = matchMedia('(max-width: 900px)');
const COLLAPSED_KEY = 'mars.sidebarCollapsed';
const WIDTH_KEY = 'mars.sidebarWidth';
let renaming: string | null = null;
let focusId: string | null = null;

// ---- Collapse, overlay, and width ----
function setCollapsed(collapsed: boolean, remember = true) {
    document.body.classList.toggle('sb-collapsed', collapsed);
    $('navToggle').setAttribute('aria-expanded', String(!collapsed));
    $('sbBackdrop').hidden = collapsed || !NARROW.matches;
    if (remember && !NARROW.matches) try { localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : ''); } catch {}
}
const isCollapsed = () => document.body.classList.contains('sb-collapsed');
export const openSidebar = () => setCollapsed(false);
const closeOverlay = () => { if (NARROW.matches) setCollapsed(true, false); };

let storedCollapsed = false;
try { storedCollapsed = localStorage.getItem(COLLAPSED_KEY) === '1'; } catch {}
setCollapsed(NARROW.matches || storedCollapsed, false);
NARROW.addEventListener('change', () => setCollapsed(NARROW.matches || storedCollapsed, false));
$('navToggle').addEventListener('click', () => setCollapsed(!isCollapsed()));
$('sbBackdrop').addEventListener('click', () => setCollapsed(true, false));

const setWidth = (px: number) => {
    const w = Math.round(Math.min(Math.max(px, 220), Math.min(560, innerWidth * 0.5)));
    document.documentElement.style.setProperty('--sb-w', `${w}px`);
    return w;
};
try { const w = Number(localStorage.getItem(WIDTH_KEY)); if (w) setWidth(w); } catch {}
const resizer = $('sbResizer');
resizer.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    resizer.setPointerCapture(e.pointerId);
    const startX = e.clientX, start = $('sidebar').offsetWidth;
    resizer.onpointermove = (ev) => { const w = setWidth(start + ev.clientX - startX); try { localStorage.setItem(WIDTH_KEY, String(w)); } catch {} };
});
resizer.addEventListener('pointerup', () => { resizer.onpointermove = null; });

// ---- Rendering ----
const filterText = () => filterInput.value.trim().toLowerCase();

function visibleRows(): { n: TreeNode; depth: number }[] {
    const rows: { n: TreeNode; depth: number }[] = [];
    const walk = (parentId: string | null, depth: number) => {
        for (const n of childrenOf(app.state.nodes, parentId)) {
            rows.push({ n, depth });
            if (n.kind === 'folder' && n.open) walk(n.id, depth + 1);
        }
    };
    walk(null, 0);
    return rows;
}

// Every document in tree order, open folders or not.
function allDocs() {
    const out: TreeNode[] = [];
    const walk = (parentId: string | null) => {
        for (const n of childrenOf(app.state.nodes, parentId)) n.kind === 'doc' ? out.push(n) : walk(n.id);
    };
    walk(null);
    return out;
}

const folderPath = (n: TreeNode) => ancestors(app.state.nodes, n.id).map((f) => f.name).join(' / ');

// With a query, the sidebar lists matching documents, best first: a match in the name or
// folder, then in the text (with a snippet), then by meaning (smart search, marked with
// sparkles). A document appears once, at its best rank.
interface Result { n: TreeNode; snippet?: string; start?: number; smart?: boolean }
let textHits = new Map<string, { snippet: string; start?: number }>();
function searchResults(q: string) {
    textHits = textMatches(q);
    const named: Result[] = [], texted: Result[] = [];
    for (const n of allDocs()) {
        if (`${n.name}\n${folderPath(n)}`.toLowerCase().includes(q)) named.push({ n });
        else if (textHits.has(n.id)) texted.push({ n, ...textHits.get(n.id)! });
    }
    const seen = new Set([...named, ...texted].map((r) => r.n.id));
    const smart = smartMatches(q);
    const meaning = (smart?.hits ?? []).flatMap((h): Result[] => {
        const n = app.node(h.id);
        return n && !seen.has(n.id) ? [{ n, snippet: h.text, start: h.start, smart: true }] : [];
    });
    const note = smart?.pending ? 'Searching by meaning…' : smart?.error ? `Smart search failed: ${smart.error}` : null;
    return { rows: [...named, ...texted, ...meaning], note };
}

const labelHtml = (n: TreeNode) => (renaming === n.id
    ? `<input class="tree-rename" value="${esc(n.name)}" aria-label="Name" spellcheck="false">`
    : `<span class="tree-label" title="${esc(n.name)}${n.handle ? ` · linked to ${esc(n.handle.name)}` : ''}">${esc(n.name)}</span>`);
const badgesHtml = (n: TreeNode) => `${n.pending ? '<span class="tree-dot" title="Changed since your last review" aria-label="Changed since your last review"></span>' : ''}${n.threads ? `<span class="tree-badge" title="${n.threads} open thread${n.threads === 1 ? '' : 's'}">${n.threads}</span>` : ''}`;
const moreHtml = (n: TreeNode) => `<button type="button" class="tree-more" tabindex="-1" aria-label="Actions for ${esc(n.name)}" title="Actions">${icon('boxesVertical')}</button>`;

function rowHtml({ n, depth }: { n: TreeNode; depth: number }) {
    const folder = n.kind === 'folder';
    const hasKids = folder && app.state.nodes.some((c) => c.parentId === n.id);
    const selected = n.id === app.state.currentId || (folder && !app.state.currentId && n.id === app.state.folderId);
    const chevron = `<span class="tree-chevron${hasKids ? '' : ' leaf'}">${icon(n.open ? 'chevronDown' : 'chevronRight')}</span>`;
    const glyph = `<span class="tree-icon">${icon(folder ? (n.open ? 'folderOpen' : 'folder') : 'document')}</span>`;
    return `<div class="tree-row" role="treeitem" data-id="${n.id}" data-kind="${n.kind}" style="--depth:${depth}" tabindex="-1" draggable="${renaming === n.id ? 'false' : 'true'}"
${folder ? `aria-expanded="${!!n.open}"` : ''} aria-selected="${selected}" aria-level="${depth + 1}">${folder ? chevron : '<span class="tree-chevron leaf"></span>'}${glyph}${labelHtml(n)}${folder ? '' : badgesHtml(n)}${moreHtml(n)}</div>`;
}

function resultHtml(r: Result) {
    const path = folderPath(r.n);
    const glyph = r.smart
        ? `<span class="tree-icon smart" title="Matches by meaning" aria-label="Matches by meaning">${icon('sparkles')}</span>`
        : `<span class="tree-icon">${icon('document')}</span>`;
    return `<div class="tree-row result${r.snippet ? ' has-snippet' : ''}" role="treeitem" data-id="${r.n.id}" data-kind="doc"${r.start === undefined ? '' : ` data-start="${r.start}"`} tabindex="-1" draggable="false" aria-selected="${r.n.id === app.state.currentId}" aria-level="1">${glyph}${labelHtml(r.n)}${path ? `<span class="tree-path" title="${esc(path)}">${esc(path)}</span>` : ''}${badgesHtml(r.n)}${moreHtml(r.n)}${r.snippet ? `<span class="tree-snippet">${esc(r.snippet)}</span>` : ''}</div>`;
}

// A render while a rename is in progress replaces the input; the typed name and the
// selection carry over, and the focusout from removing the old input doesn't commit it.
let rendering = false;
export function render() {
    const q = filterText();
    const typing = tree.querySelector<HTMLInputElement>('.tree-rename');
    const draft = typing && { value: typing.value, start: typing.selectionStart, end: typing.selectionEnd };
    // Background renders (saves, other tabs, search) keep keyboard focus on the same row.
    const focusedRow = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>('#tree .tree-row')?.dataset.id;
    rendering = true;
    try {
        if (!app.state.nodes.length) {
            tree.innerHTML = `<div class="tree-empty">No documents yet. Open a file, start writing, or open a shared link.<br><button type="button" class="btn btn-sm" data-new-doc>New document</button></div>`;
        } else if (q) {
            const { rows, note } = searchResults(q);
            const noteHtml = note ? `<div class="tree-note">${esc(note)}</div>` : '';
            tree.innerHTML = rows.length ? rows.map(resultHtml).join('') + noteHtml : noteHtml || '<div class="tree-empty">No documents match.</div>';
        } else {
            tree.innerHTML = visibleRows().map(rowHtml).join('');
        }
    } finally {
        rendering = false;
    }
    const input = tree.querySelector<HTMLInputElement>('.tree-rename');
    if (input) {
        input.focus();
        if (draft) { input.value = draft.value; input.setSelectionRange(draft.start, draft.end); } else input.select();
    } else if (focusedRow) rowEl(focusedRow)?.focus();
    const focus = rowEl(focusId ?? app.state.currentId);
    if (focus) focus.tabIndex = 0; else (tree.querySelector('.tree-row') as HTMLElement | null)?.setAttribute('tabindex', '0');
    renderCrumbs();
    renderUsage();
}

const rowEl = (id: string | null) => (id ? tree.querySelector<HTMLElement>(`.tree-row[data-id="${id}"]`) : null);
const rowNode = (el: Element | null) => app.node((el?.closest('.tree-row') as HTMLElement | null)?.dataset.id ?? null);

function renderCrumbs() {
    const cur = app.current();
    const chain = cur ? ancestors(app.state.nodes, cur.id) : [];
    $('crumbs').innerHTML = chain.map((f) => `<li><button type="button" data-folder="${f.id}" title="${esc(f.name)}">${esc(f.name)}</button></li>`).join('');
}
$('crumbs').addEventListener('click', (e) => {
    const id = (e.target as HTMLElement).closest<HTMLElement>('[data-folder]')?.dataset.folder;
    if (!id) return;
    openSidebar();
    app.state.folderId = id;
    app.setOpen(id, true);
    requestAnimationFrame(() => rowEl(id)?.scrollIntoView({ block: 'nearest' }));
});

let usageTimer: ReturnType<typeof setTimeout> | undefined;
function renderUsage() {
    clearTimeout(usageTimer);
    usageTimer = setTimeout(async () => {
        // The size of the documents themselves; storage.estimate() covers the whole origin.
        const docs = app.state.nodes.filter((n) => n.kind === 'doc');
        for (const n of docs) n.size ??= textBytes((await db.getText(n.id)) ?? '');
        const bytes = docs.reduce((sum, n) => sum + (n.size ?? 0), 0);
        const size = !docs.length ? '' : ` · ${bytes < 1048576 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / 1048576).toFixed(1)} MB`}`;
        $('sbUsage').textContent = `${docs.length} document${docs.length === 1 ? '' : 's'}${size}`;
    }, 300);
}

// ---- Actions ----
async function newDoc(parentId: string | null = app.state.folderId) {
    const n = await app.createDoc({ name: app.uniqueName('Untitled', parentId), parentId });
    setViewMode('edit');
    closeOverlay();
    return n;
}

async function newFolder(parentId: string | null = app.state.folderId) {
    const n = await app.createFolder(app.uniqueName('New folder', parentId), parentId);
    app.state.folderId = n.id;
    startRename(n.id);
}

function startRename(id: string) {
    renaming = id;
    render();
}

async function finishRename(commit: boolean) {
    const input = tree.querySelector<HTMLInputElement>('.tree-rename');
    const id = renaming;
    renaming = null;
    if (id && input && commit) await app.rename(id, input.value);
    render();
    if (id) { focusId = id; rowEl(id)?.focus(); }
}

async function deleteNode(n: TreeNode) {
    const kids = app.state.nodes.filter((c) => c.parentId === n.id).length;
    if (n.kind === 'folder' && kids && !(await confirmDanger(`Delete "${n.name}"?`, 'The folder and everything in it are removed from this browser. Export it first if you need a copy.', 'Delete folder'))) return;
    const undo = await app.remove(n.id);
    if (undo) toast(`Deleted ${n.name}`, { label: 'Undo', run: undo });
}

// A document tied to its source: a linked file, a recorded source text, or a file or URL origin.
const linked = (n: TreeNode) => !!(n.handle || n.opened || /^(file|url):/.test(n.originId ?? ''));

async function unlinkDoc(n: TreeNode) {
    const source = n.handle?.name ?? n.originId?.replace(/^(file|url):/, '') ?? 'its source';
    const { value } = await modal({
        title: `Unlink "${n.name}"?`,
        html: `<p>MARS forgets that this document came from <strong>${esc(source)}</strong>. The text and comments stay.</p><p>Opening that source again adds a new document instead of updating this one, and "changes since opened" and pending change marks go away.</p>`,
        buttons: [{ label: 'Cancel', value: '' }, { label: 'Unlink', value: 'ok', primary: true }],
    });
    if (value !== 'ok') return;
    await app.unlink(n.id);
    toast(`Unlinked "${n.name}"`);
}

function menuFor(n: TreeNode): MenuEntry[] {
    const dirPicker = 'showDirectoryPicker' in window;
    if (n.kind === 'folder') return [
        { label: 'New document', icon: 'document', run: () => newDoc(n.id) },
        { label: 'New folder', icon: 'folder', run: () => newFolder(n.id) },
        { label: 'Rename', icon: 'pencil', run: () => startRename(n.id) },
        'separator',
        { label: 'Copy short link', icon: 'link', sub: 'Encrypted copy of the whole folder', run: () => share.copyNodeLink(n.id, 'short') },
        { label: 'Copy full link', icon: 'documents', sub: 'The folder inside the URL', run: () => share.copyNodeLink(n.id, 'full') },
        { label: 'Download .zip', icon: 'download', run: () => io.exportZip(n.id) },
        ...(dirPicker ? [{ label: 'Export to a folder on disk…', icon: 'save' as const, run: () => io.exportToDirectory(n.id) }] : []),
        'separator',
        { label: 'Delete', icon: 'trash', danger: true, run: () => deleteNode(n) },
    ];
    return [
        { label: 'Rename', icon: 'pencil', run: () => startRename(n.id) },
        { label: 'Duplicate', icon: 'copy', run: () => app.duplicate(n.id) },
        ...(n.handle ? [{ label: 'Reload from disk', icon: 'refresh' as const, sub: n.handle.name, run: () => app.reloadFromDisk(n.id) }] : []),
        ...(linked(n) ? [{ label: 'Unlink from source', icon: 'cross' as const, run: () => unlinkDoc(n) }] : []),
        ...(n.parentId ? [{ label: 'Move to top level', icon: 'home' as const, run: () => app.move(n.id, null) }] : []),
        'separator',
        { label: 'Copy short link', icon: 'link', run: () => share.copyNodeLink(n.id, 'short') },
        { label: 'Copy full link', icon: 'documents', run: () => share.copyNodeLink(n.id, 'full') },
        { label: 'Download .md', icon: 'download', run: () => io.downloadDoc(n.id) },
        'separator',
        { label: 'Delete', icon: 'trash', danger: true, run: () => deleteNode(n) },
    ];
}

// A search result opens at its match: the passage found by meaning, or Find on the query.
async function activate(n: TreeNode, row?: HTMLElement | null) {
    focusId = n.id;
    if (n.kind === 'folder') {
        app.state.folderId = n.id;
        await app.setOpen(n.id, !n.open);
        return;
    }
    const q = filterText(), start = row?.dataset.start;
    if (n.id !== app.state.currentId) await app.openDoc(n.id);
    if (q && start !== undefined) viewer.reveal({ start: Number(start) });
    else if (q && textHits.has(n.id)) viewer.reveal({ query: q });
    closeOverlay();
}

tree.addEventListener('click', (e) => {
    const target = e.target as HTMLElement;
    if (target.closest('[data-new-doc]')) { newDoc(null); return; }
    if (target.closest('.tree-rename')) return;
    const n = rowNode(target);
    if (!n) return;
    if (target.closest('.tree-more')) { openMenu(target.closest('.tree-more')!, menuFor(n), `Actions for ${n.name}`); return; }
    if (target.closest('.tree-chevron') && n.kind === 'folder') { app.setOpen(n.id, !n.open); return; }
    activate(n, target.closest<HTMLElement>('.tree-row'));
});
tree.addEventListener('dblclick', (e) => {
    const n = rowNode(e.target as HTMLElement);
    if (n && !(e.target as HTMLElement).closest('.tree-more, .tree-chevron, .tree-rename')) startRename(n.id);
});
tree.addEventListener('contextmenu', (e) => {
    const n = rowNode(e.target as HTMLElement);
    if (!n || renaming) return;
    e.preventDefault();
    openMenu({ x: e.clientX, y: e.clientY }, menuFor(n), `Actions for ${n.name}`);
});
tree.addEventListener('keydown', (e) => {
    if ((e.target as HTMLElement).matches('.tree-rename')) {
        if (e.key === 'Enter') { e.preventDefault(); finishRename(true); }
        else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); finishRename(false); }
        return;
    }
    const rows = [...tree.querySelectorAll<HTMLElement>('.tree-row')];
    const el = (document.activeElement as HTMLElement)?.closest<HTMLElement>('.tree-row') ?? rows.find((r) => r.tabIndex === 0) ?? rows[0];
    const n = rowNode(el);
    if (!el || !n) return;
    const go = (to: HTMLElement | undefined) => { if (!to) return; el.tabIndex = -1; to.tabIndex = 0; to.focus(); focusId = to.dataset.id ?? null; };
    const i = rows.indexOf(el);
    switch (e.key) {
        case 'ArrowDown': e.preventDefault(); go(rows[i + 1]); break;
        case 'ArrowUp': e.preventDefault(); go(rows[i - 1]); break;
        case 'Home': e.preventDefault(); go(rows[0]); break;
        case 'End': e.preventDefault(); go(rows.at(-1)); break;
        case 'ArrowRight':
            e.preventDefault();
            if (n.kind === 'folder' && !n.open) app.setOpen(n.id, true); else go(rows[i + 1]);
            break;
        case 'ArrowLeft':
            e.preventDefault();
            if (n.kind === 'folder' && n.open) app.setOpen(n.id, false);
            else if (n.parentId) go(rowEl(n.parentId) ?? undefined);
            break;
        case 'Enter': case ' ': e.preventDefault(); activate(n, el); break;
        case 'F2': e.preventDefault(); startRename(n.id); break;
        case 'Delete': case 'Backspace': e.preventDefault(); deleteNode(n); break;
        case 'ContextMenu': e.preventDefault(); openMenu(el, menuFor(n), `Actions for ${n.name}`); break;
    }
});
tree.addEventListener('focusout', (e) => {
    if ((e.target as HTMLElement).matches('.tree-rename') && renaming && !rendering) finishRename(true);
});

$('sbNewBtn').addEventListener('click', (e) => openMenu(e.currentTarget as HTMLElement, [
    { label: 'New document', icon: 'document', run: () => newDoc() },
    { label: 'New folder', icon: 'folder', run: () => newFolder() },
    'separator',
    { label: 'Import files…', icon: 'upload', sub: '.md files or a .zip export', run: () => $('importInput').click() },
    { label: 'Import a folder…', icon: 'folderOpen', run: () => $('importFolderInput').click() },
], 'New'));
$('sbExportAllBtn').addEventListener('click', () => io.exportZip(null));
filterInput.addEventListener('input', render);
filterInput.addEventListener('keydown', (e) => { if (e.key === 'Escape' && filterInput.value) { e.stopPropagation(); filterInput.value = ''; render(); } });

// ---- Drag and drop: move rows, or import dropped files ----
type Drop = { parentId: string | null; beforeId: string | null; row: HTMLElement | null; cls: string };
let dragId: string | null = null;
let lastDrop: Drop | null = null;

function dropTarget(e: DragEvent): Drop {
    const row = (e.target as HTMLElement).closest<HTMLElement>('.tree-row');
    const n = rowNode(row);
    if (!row || !n) return { parentId: null, beforeId: null, row: null, cls: '' };
    const r = row.getBoundingClientRect();
    const y = (e.clientY - r.top) / r.height;
    const siblings = childrenOf(app.state.nodes, n.parentId).filter((s) => s.id !== dragId);
    const after = siblings[siblings.findIndex((s) => s.id === n.id) + 1]?.id ?? null;
    if (n.kind === 'folder' && y > 0.25 && y < 0.75) return { parentId: n.id, beforeId: null, row, cls: 'drop-inside' };
    if (y < 0.5) return { parentId: n.parentId, beforeId: n.id, row, cls: 'drop-before' };
    // Below an open folder, a drop goes to its first child slot.
    if (n.kind === 'folder' && n.open && app.state.nodes.some((c) => c.parentId === n.id)) {
        return { parentId: n.id, beforeId: childrenOf(app.state.nodes, n.id)[0]?.id ?? null, row, cls: 'drop-after' };
    }
    return { parentId: n.parentId, beforeId: after, row, cls: 'drop-after' };
}

function clearDrop() {
    lastDrop?.row?.classList.remove('drop-inside', 'drop-before', 'drop-after');
    tree.classList.remove('drop-root');
    lastDrop = null;
}

tree.addEventListener('dragstart', (e) => {
    const n = rowNode(e.target as HTMLElement);
    if (!n || !e.dataTransfer) return;
    dragId = n.id;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('application/x-mars-node', n.id);
    e.dataTransfer.setData('text/plain', n.name);
    rowEl(n.id)?.classList.add('dragging');
});
tree.addEventListener('dragend', () => { rowEl(dragId)?.classList.remove('dragging'); dragId = null; clearDrop(); });
tree.addEventListener('dragover', (e) => {
    const files = e.dataTransfer?.types.includes('Files');
    if (!dragId && !files) return;
    const drop = dropTarget(e);
    if (dragId && !canMove(app.state.nodes, dragId, drop.parentId)) { clearDrop(); return; }
    e.preventDefault();
    e.dataTransfer!.dropEffect = dragId ? 'move' : 'copy';
    if (lastDrop?.row !== drop.row || lastDrop?.cls !== drop.cls) {
        clearDrop();
        drop.row?.classList.add(drop.cls);
        if (!drop.row) tree.classList.add('drop-root');
        lastDrop = drop;
    }
});
tree.addEventListener('dragleave', (e) => { if (!tree.contains(e.relatedTarget as Node)) clearDrop(); });
tree.addEventListener('drop', async (e) => {
    e.preventDefault();
    const drop = dropTarget(e);
    clearDrop();
    if (dragId) {
        const id = dragId;
        dragId = null;
        await app.move(id, drop.parentId, drop.beforeId);
        return;
    }
    if (e.dataTransfer) await io.importDataTransfer(e.dataTransfer, drop.parentId);
});

app.onChange(render);
onUpdate(render);
