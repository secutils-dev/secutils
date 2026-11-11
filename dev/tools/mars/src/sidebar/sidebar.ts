// Documents sidebar: a docked, hideable tree of folders and documents (EuiCollapsibleNav +
// EuiTreeView look). Drag rows to move or reorder them; drop files to import them.
import * as app from '../app.ts';
import { ancestors, canMove, childrenOf, descendants, textBytes, type TreeNode } from '../tree/model.ts';
import { icon, folderIcon } from '../ui/icons.ts';
import { $, closeMenu, esc, modal, openMenu, type MenuEntry } from '../ui/ui.ts';
import * as viewer from '../doc/viewer.js';
import { setViewMode, toast } from '../doc/viewer.js';
import { onUpdate, smartMatches, textMatches } from '../search/search.ts';
import * as io from '../io/io.ts';
import * as share from '../share/share.ts';
import * as db from '../store/db.ts';
import * as sync from '../sync/index.ts';
import { incomingFolder } from '../io/destination.ts';

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
    if (remember && !NARROW.matches)
        try {
            localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '');
        } catch {}
}
const isCollapsed = () => document.body.classList.contains('sb-collapsed');
export const openSidebar = () => setCollapsed(false);
const closeOverlay = () => {
    if (NARROW.matches) setCollapsed(true, false);
};

let storedCollapsed = false;
try {
    storedCollapsed = localStorage.getItem(COLLAPSED_KEY) === '1';
} catch {}
setCollapsed(NARROW.matches || storedCollapsed, false);
NARROW.addEventListener('change', () => setCollapsed(NARROW.matches || storedCollapsed, false));
$('navToggle').addEventListener('click', () => setCollapsed(!isCollapsed()));
$('sbBackdrop').addEventListener('click', () => setCollapsed(true, false));

const setWidth = (px: number) => {
    const w = Math.round(Math.min(Math.max(px, 220), Math.min(560, innerWidth * 0.5)));
    document.documentElement.style.setProperty('--sb-w', `${w}px`);
    return w;
};
try {
    const w = Number(localStorage.getItem(WIDTH_KEY));
    if (w) setWidth(w);
} catch {}
const resizer = $('sbResizer');
resizer.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    resizer.setPointerCapture(e.pointerId);
    const startX = e.clientX,
        start = $('sidebar').offsetWidth;
    resizer.onpointermove = (ev) => {
        const w = setWidth(start + ev.clientX - startX);
        try {
            localStorage.setItem(WIDTH_KEY, String(w));
        } catch {}
    };
});
resizer.addEventListener('pointerup', () => {
    resizer.onpointermove = null;
});

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

const folderPath = (n: TreeNode) =>
    ancestors(app.state.nodes, n.id)
        .map((f) => f.name)
        .join(' / ');

// With a query, the sidebar lists matching documents, best first: a match in the name or
// folder, then in the text (with a snippet), then by meaning (smart search, marked with
// sparkles). A document appears once, at its best rank.
interface Result {
    n: TreeNode;
    snippet?: string;
    start?: number;
    smart?: boolean;
}
let textHits = new Map<string, { snippet: string; start?: number }>();
function searchResults(q: string) {
    textHits = textMatches(q);
    const named: Result[] = [],
        texted: Result[] = [];
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

const connectionHtml = (n: TreeNode) =>
    n.connection
        ? `<button type="button" class="tree-connection" title="${esc(sync.label(n))}" data-connection="${n.id}" data-phase="${n.connection.phase ?? 'waiting'}" aria-label="${esc(n.connection.phase === 'ended' ? 'Local document' : n.connection.kind === 'live' ? 'Live document' : n.connection.liveShared ? 'Shared document' : 'Sync')}: ${esc(sync.label(n))}">${icon(n.connection.phase === 'ended' ? 'unlink' : n.connection.kind === 'live' || n.connection.liveShared ? 'link' : 'refresh')}</button>`
        : '';
const defaultFolder = () =>
    app.defaultIncomingFolder() ??
    app.state.nodes.find(
        (node) => node.kind === 'folder' && node.parentId === null && node.name === share.SHARED_FOLDER,
    );
let renderedDefaultFolder: string | undefined;
const isDefaultFolder = (n: TreeNode) => n.kind === 'folder' && n.id === renderedDefaultFolder;
const labelHtml = (n: TreeNode) =>
    renaming === n.id
        ? `<input class="tree-rename" value="${esc(n.name)}" aria-label="Name" spellcheck="false">`
        : `<span class="tree-label" title="${esc(n.name)}${isDefaultFolder(n) ? ' · Default destination' : ''}${n.handle ? ` · linked to ${esc(n.handle.name)}` : ''}">${esc(n.name)}</span>`;
const badgesHtml = (n: TreeNode) =>
    `${n.pending ? '<span class="tree-dot" title="Changed since your last review" aria-label="Changed since your last review"></span>' : ''}${n.threads ? `<span class="tree-badge" title="${n.threads} open thread${n.threads === 1 ? '' : 's'}">${n.threads}</span>` : ''}`;
const moreHtml = (n: TreeNode) =>
    `<button type="button" class="tree-more" tabindex="-1" aria-label="Actions for ${esc(n.name)}" title="Actions">${icon('boxesVertical')}</button>`;
// Local routes are real links, so the browser can open them in another tab/window.
// Keep action buttons and rename inputs outside the anchor.
const docLink = (n: TreeNode, content: string) =>
    db.isTemporaryStorage()
        ? `<button type="button" class="tree-link" tabindex="-1" draggable="false" aria-label="${esc(n.name)}">${content}</button>`
        : `<a class="tree-link" href="${esc(location.pathname)}#doc=${encodeURIComponent(n.id)}" tabindex="-1" draggable="false" aria-label="${esc(n.name)}"${n.id === app.state.currentId ? ' aria-current="page"' : ''}>${content}</a>`;

function rowHtml({ n, depth }: { n: TreeNode; depth: number }) {
    const folder = n.kind === 'folder';
    const defaultFolder = isDefaultFolder(n);
    const hasKids = folder && app.state.nodes.some((c) => c.parentId === n.id);
    const selected = n.id === app.state.currentId || (folder && !app.state.currentId && n.id === app.state.folderId);
    const chevron = `<span class="tree-chevron${hasKids ? '' : ' leaf'}">${icon(n.open ? 'chevronDown' : 'chevronRight')}</span>`;
    const glyph = `<span class="tree-icon${defaultFolder ? ' tree-icon-default' : ''}" aria-hidden="true">${folder ? folderIcon(!!n.open, defaultFolder) : icon('document')}</span>`;
    const content = `${folder ? chevron : '<span class="tree-chevron leaf"></span>'}${glyph}${labelHtml(n)}${folder ? '' : badgesHtml(n)}`;
    return `<div class="tree-row" role="treeitem"${defaultFolder ? ' aria-description="Default destination"' : ''} data-id="${n.id}" data-kind="${n.kind}" style="--depth:${depth}" tabindex="-1" draggable="${renaming === n.id ? 'false' : 'true'}"
${folder ? `aria-expanded="${!!n.open}"` : ''} aria-selected="${selected}" aria-level="${depth + 1}">${!folder && renaming !== n.id ? docLink(n, content) : content}${connectionHtml(n)}${moreHtml(n)}</div>`;
}

function resultHtml(r: Result) {
    const path = folderPath(r.n);
    const glyph = r.smart
        ? `<span class="tree-icon smart" title="Matches by meaning" aria-label="Matches by meaning">${icon('sparkles')}</span>`
        : `<span class="tree-icon">${icon('document')}</span>`;
    const content = `${glyph}${labelHtml(r.n)}${path ? `<span class="tree-path" title="${esc(path)}">${esc(path)}</span>` : ''}${badgesHtml(r.n)}${r.snippet ? `<span class="tree-snippet">${esc(r.snippet)}</span>` : ''}`;
    return `<div class="tree-row result${r.snippet ? ' has-snippet' : ''}" role="treeitem" data-id="${r.n.id}" data-kind="doc"${r.start === undefined ? '' : ` data-start="${r.start}"`} tabindex="-1" draggable="false" aria-selected="${r.n.id === app.state.currentId}" aria-level="1">${renaming !== r.n.id ? docLink(r.n, content) : content}${connectionHtml(r.n)}${moreHtml(r.n)}</div>`;
}

// A render while a rename is in progress replaces the input; the typed name and the
// selection carry over, and the focusout from removing the old input doesn't commit it.
let rendering = false;
let linkGesture = false,
    renderPending = false;
export function render() {
    // A save must not replace the anchor between pointerdown and its native navigation.
    if (linkGesture) {
        renderPending = true;
        return;
    }
    renderedDefaultFolder = defaultFolder()?.id;
    const q = filterText();
    const typing = tree.querySelector<HTMLInputElement>('.tree-rename');
    const draft = typing && { value: typing.value, start: typing.selectionStart, end: typing.selectionEnd };
    // Background renders (saves, other tabs, search) keep keyboard focus on the same row.
    const focusedRow = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>('#tree .tree-row')?.dataset
        .id;
    rendering = true;
    try {
        if (!app.state.nodes.length) {
            tree.innerHTML = `<div class="tree-empty">No documents yet. Open a file, start writing, or open a shared link.<br><button type="button" class="btn btn-sm" data-new-doc>New document</button></div>`;
        } else if (q) {
            const { rows, note } = searchResults(q);
            const noteHtml = note ? `<div class="tree-note">${esc(note)}</div>` : '';
            tree.innerHTML = rows.length
                ? rows.map(resultHtml).join('') + noteHtml
                : noteHtml || '<div class="tree-empty">No documents match.</div>';
        } else {
            tree.innerHTML = visibleRows().map(rowHtml).join('');
        }
        // Rows are rebuilt during Sync. Use the document timeline so replacement
        // icons continue their full turn instead of restarting on every update.
        for (const glyph of tree.querySelectorAll('.tree-connection[data-phase="working"] svg')) {
            for (const animation of glyph.getAnimations())
                if (animation instanceof CSSAnimation && animation.animationName === 'sync-turn')
                    animation.startTime = 0;
        }
    } finally {
        rendering = false;
    }
    const input = tree.querySelector<HTMLInputElement>('.tree-rename');
    if (input) {
        input.focus();
        if (draft) {
            input.value = draft.value;
            input.setSelectionRange(draft.start, draft.end);
        } else input.select();
    } else if (focusedRow) focusTarget(rowEl(focusedRow))?.focus();
    const focus = focusTarget(rowEl(focusId ?? app.state.currentId) ?? tree.querySelector<HTMLElement>('.tree-row'));
    if (focus) focus.tabIndex = 0;
    renderCrumbs();
    renderUsage();
}

const rowEl = (id: string | null) => (id ? tree.querySelector<HTMLElement>(`.tree-row[data-id="${id}"]`) : null);
const focusTarget = (row: HTMLElement | null) => row?.querySelector<HTMLAnchorElement>('.tree-link') ?? row;
const rowNode = (el: Element | null) => app.node((el?.closest('.tree-row') as HTMLElement | null)?.dataset.id ?? null);

function renderCrumbs() {
    const cur = app.current();
    const chain = cur ? ancestors(app.state.nodes, cur.id) : [];
    $('crumbs').innerHTML = chain
        .map(
            (f) =>
                `<li><button type="button" data-folder="${f.id}" title="${esc(f.name)}">${esc(f.name)}</button></li>`,
        )
        .join('');
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
        const size = !docs.length
            ? ''
            : ` · ${bytes < 1048576 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / 1048576).toFixed(1)} MB`}`;
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
    if (id) {
        focusId = id;
        focusTarget(rowEl(id))?.focus();
    }
}

const deleting = new Set<string>();
async function deleteNode(n: TreeNode) {
    if (deleting.has(n.id)) return;
    const current = app.node(n.id);
    if (!current) return;
    n = current;
    deleting.add(n.id);
    try {
        const selected = [n, ...(n.kind === 'folder' ? descendants(app.state.nodes, n.id) : [])];
        const ids = selected.map((node) => node.id),
            connected = selected.some((node) => node.connection);
        const shortLinks = share.shortLinksForDeletion(ids);
        const info = connected ? await sync.deletionInfo(ids) : { liveLinks: 0, synced: 0 };
        const active = selected.filter((node) => node.connection && node.connection.phase !== 'ended');
        const remote = n.connection?.permission === 'owner' && n.connection.phase !== 'ended';
        const docs = selected.filter((node) => node.kind === 'doc').length;
        const paragraphs = [
            n.kind === 'folder'
                ? `This removes the folder and ${docs} document${docs === 1 ? '' : 's'} from this browser.`
                : 'This removes the document from this browser.',
        ];
        if (info.synced)
            paragraphs.push(
                `${info.synced === 1 ? 'This document syncs' : `${info.synced} documents sync`} across devices. Deleting the local copy leaves other devices connected.`,
            );
        if (active.some((node) => node.connection!.kind === 'live'))
            paragraphs.push("This copy receives Live updates. Deleting it does not disable the owner's link.");
        if (connected && !active.length)
            paragraphs.push('The connection has ended. Only the saved local copy will be removed.');
        if (info.liveLinks)
            paragraphs.push(
                `${info.liveLinks} active Live link${info.liveLinks === 1 ? '' : 's'}. Deleting the local copy leaves ${info.liveLinks === 1 ? 'it' : 'them'} active.${remote ? ' Deleting from the collection ends Live access. Saved copies on other devices remain.' : ''}`,
            );
        if (remote && (!info.liveLinks || info.synced))
            paragraphs.push(
                'Delete from collection also ends access on other devices. Saved copies on those devices remain.' +
                    (n.kind === 'folder' ? ' Local-only items inside the folder are kept at the top level.' : ''),
            );
        if (shortLinks)
            paragraphs.push(
                `${shortLinks} active short snapshot link${shortLinks === 1 ? '' : 's'} ${shortLinks === 1 ? 'contains' : 'contain'} this content. ${shortLinks === 1 ? 'It stays' : 'They stay'} available until expiry. To delete these links first, cancel and use Share on the shared item.`,
            );
        const answer = await modal({
            title: `Delete "${n.name}"?`,
            html: paragraphs.map((text) => `<p>${esc(text)}</p>`).join(''),
            buttons: [
                { label: 'Cancel', value: '' },
                {
                    label: active.length
                        ? 'Delete local copy'
                        : n.kind === 'folder'
                          ? 'Delete folder'
                          : 'Delete document',
                    value: 'local',
                    danger: true,
                },
                ...(remote ? [{ label: 'Delete from collection', value: 'remote', danger: true }] : []),
            ],
        });
        if (!answer.value) return;
        if (connected) toast(`Deleting ${n.name}…`);
        await viewer.flush();
        if (connected) await sync.remove(n.id, answer.value === 'remote');
        const undo = await app.remove(n.id);
        if (undo) toast(`Deleted ${n.name}`, { label: 'Undo', run: undo });
    } catch (error) {
        toast(`Could not delete "${n.name}": ${(error as Error).message}. Your local copy is still here.`, {
            label: 'Try again',
            run: () => deleteNode(n),
        });
    } finally {
        deleting.delete(n.id);
    }
}

// A document tied to its source: a linked file, a recorded source text, or a file or URL origin.
const linked = (n: TreeNode) => !!(n.handle || n.opened || /^(file|url):/.test(n.originId ?? ''));

async function unlinkDoc(n: TreeNode) {
    if (n.connection) {
        await sync.unlink(n.id);
        return;
    }
    const { value } = await modal({
        title: 'Make this document independent?',
        body: 'This document and its comments stay here. Opening the original file or link again will not automatically update this document.',
        buttons: [
            { label: 'Cancel', value: '' },
            { label: 'Make independent', value: 'ok', primary: true },
        ],
    });
    if (value !== 'ok') return;
    await app.unlink(n.id);
    toast(`"${n.name}" is now an independent document`);
}

function menuFor(n: TreeNode): MenuEntry[] {
    const dirPicker = 'showDirectoryPicker' in window;
    const editable = !n.connection || ['owner', 'edit'].includes(n.connection.permission);
    if (n.kind === 'folder')
        return [
            { label: 'New document', icon: 'document', run: () => newDoc(n.id) },
            { label: 'New folder', icon: 'folder', run: () => newFolder(n.id) },
            {
                label: 'Use as default',
                icon: 'folder',
                disabled:
                    db.isTemporaryStorage() || defaultFolder()?.id === n.id || !incomingFolder(app.state.nodes, n.id),
                run: async () => {
                    try {
                        await app.setDefaultIncomingFolder(n.id);
                        toast(
                            `New files and links will go to "${n.name}".${n.connection ? ' New documents will sync too.' : ''}`,
                        );
                    } catch {
                        toast('Could not save the default destination. Try again.');
                    }
                },
            },
            { label: 'Rename', icon: 'pencil', disabled: !editable, run: () => startRename(n.id) },
            {
                label: 'Sync',
                icon: 'refresh',
                disabled: db.isTemporaryStorage() && !n.connection,
                run: () => sync.enroll(n.id).catch((e) => toast(e.message)),
            },
            'separator',
            { label: 'Copy link', icon: 'link', run: () => share.copyLinkDialog(n.id) },
            { label: 'Download', icon: 'download', run: () => io.exportZip(n.id) },
            ...(dirPicker
                ? [
                      {
                          label: 'Export to a folder on disk…',
                          icon: 'save' as const,
                          run: () => io.exportToDirectory(n.id),
                      },
                  ]
                : []),
            'separator',
            { label: 'Delete', icon: 'trash', danger: true, run: () => deleteNode(n) },
        ];
    return [
        { label: 'Rename', icon: 'pencil', disabled: !editable, run: () => startRename(n.id) },
        { label: 'Duplicate', icon: 'copy', run: () => app.duplicate(n.id) },
        {
            label: n.connection?.kind === 'live' ? 'Live document' : 'Sync',
            icon: 'refresh',
            disabled: db.isTemporaryStorage() && !n.connection,
            run: () =>
                n.connection?.kind === 'live'
                    ? sync.details(n.id).catch((e) => toast(e.message))
                    : sync.enroll(n.id).catch((e) => toast(e.message)),
        },
        ...(n.handle
            ? [
                  {
                      label: 'Reload from disk',
                      icon: 'refresh' as const,
                      sub: n.handle.name,
                      run: () => app.reloadFromDisk(n.id),
                  },
              ]
            : []),
        ...(linked(n) && !n.connection
            ? [{ label: 'Make independent', icon: 'cross' as const, run: () => unlinkDoc(n) }]
            : []),
        'separator',
        { label: 'Copy link', icon: 'link', run: () => share.copyLinkDialog(n.id) },
        { label: 'Download', icon: 'download', run: () => io.downloadDoc(n.id) },
        'separator',
        { label: 'Delete', icon: 'trash', danger: true, run: () => deleteNode(n) },
        ...(!db.isTemporaryStorage() ? [{ hint: 'Shift + right-click for browser actions' }] : []),
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
    const q = filterText(),
        start = row?.dataset.start;
    if (n.id !== app.state.currentId) await app.openDoc(n.id);
    if (q && start !== undefined) viewer.reveal({ start: Number(start) });
    else if (q && textHits.has(n.id)) viewer.reveal({ query: q });
    closeOverlay();
}

tree.addEventListener('pointerdown', (e) => {
    if (!(e.target as HTMLElement).closest('.tree-link')) return;
    linkGesture = true;
    // Start the IDB write before a new tab reads this document, without canceling the gesture.
    viewer.flush();
});
const endLinkGesture = () => {
    if (!linkGesture) return;
    // Native click/auxclick follows pointerup in the same task. Keep its anchor until then.
    setTimeout(() => {
        linkGesture = false;
        if (renderPending) {
            renderPending = false;
            render();
        }
    }, 0);
};
document.addEventListener('pointerup', endLinkGesture);
document.addEventListener('pointercancel', endLinkGesture);
window.addEventListener('blur', endLinkGesture);

tree.addEventListener('click', (e) => {
    const target = e.target as HTMLElement;
    if (target.closest('[data-new-doc]')) {
        newDoc(null);
        return;
    }
    if (target.closest('.tree-rename')) return;
    const n = rowNode(target);
    if (!n) return;
    if (target.closest('[data-connection]')) {
        e.preventDefault();
        void sync.details(n.id);
        return;
    }
    if (target.closest('.tree-more')) {
        openMenu(target.closest('.tree-more')!, menuFor(n), `Actions for ${n.name}`);
        return;
    }
    if (target.closest('.tree-chevron') && n.kind === 'folder') {
        app.setOpen(n.id, !n.open);
        return;
    }
    if (target.closest('.tree-link')) {
        if (
            e.defaultPrevented ||
            e.button !== 0 ||
            (target.closest('a.tree-link') && (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey))
        )
            return;
        e.preventDefault();
    }
    activate(n, target.closest<HTMLElement>('.tree-row'));
});
tree.addEventListener('dblclick', (e) => {
    const n = rowNode(e.target as HTMLElement);
    if (n && !(e.target as HTMLElement).closest('.tree-more, .tree-chevron, .tree-rename')) startRename(n.id);
});
tree.addEventListener('contextmenu', (e) => {
    const n = rowNode(e.target as HTMLElement);
    if (!n || renaming) return;
    // Firefox bypasses this event for Shift + right-click. Other engines need this branch.
    if (e.shiftKey && (e.target as HTMLElement).closest('.tree-link')) {
        closeMenu();
        return;
    }
    e.preventDefault();
    openMenu({ x: e.clientX, y: e.clientY }, menuFor(n), `Actions for ${n.name}`);
});
tree.addEventListener('keydown', (e) => {
    const badge = (e.target as HTMLElement).closest<HTMLElement>('[data-connection]');
    if (badge && (e.key === 'Enter' || e.key === ' ')) {
        e.stopPropagation();
        return;
    }
    if ((e.target as HTMLElement).matches('.tree-rename')) {
        if (e.key === 'Enter') {
            e.preventDefault();
            finishRename(true);
        } else if (e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            finishRename(false);
        }
        return;
    }
    const rows = [...tree.querySelectorAll<HTMLElement>('.tree-row')];
    const el =
        (document.activeElement as HTMLElement)?.closest<HTMLElement>('.tree-row') ??
        rows.find((r) => focusTarget(r)?.tabIndex === 0) ??
        rows[0];
    const n = rowNode(el);
    if (!el || !n) return;
    const go = (to: HTMLElement | undefined) => {
        if (!to) return;
        focusTarget(el)!.tabIndex = -1;
        const target = focusTarget(to)!;
        target.tabIndex = 0;
        target.focus();
        focusId = to.dataset.id ?? null;
    };
    // Let a focused link retain the browser's modified Enter behavior.
    if (
        (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) &&
        e.key === 'Enter' &&
        (e.target as HTMLElement).closest('.tree-link')
    )
        return;
    const i = rows.indexOf(el);
    switch (e.key) {
        case 'ArrowDown':
            e.preventDefault();
            go(rows[i + 1]);
            break;
        case 'ArrowUp':
            e.preventDefault();
            go(rows[i - 1]);
            break;
        case 'Home':
            e.preventDefault();
            go(rows[0]);
            break;
        case 'End':
            e.preventDefault();
            go(rows.at(-1));
            break;
        case 'ArrowRight':
            e.preventDefault();
            if (n.kind === 'folder' && !n.open) app.setOpen(n.id, true);
            else go(rows[i + 1]);
            break;
        case 'ArrowLeft':
            e.preventDefault();
            if (n.kind === 'folder' && n.open) app.setOpen(n.id, false);
            else if (n.parentId) go(rowEl(n.parentId) ?? undefined);
            break;
        case 'Enter':
        case ' ':
            e.preventDefault();
            activate(n, el);
            break;
        case 'F2':
            e.preventDefault();
            startRename(n.id);
            break;
        case 'Delete':
        case 'Backspace':
            e.preventDefault();
            deleteNode(n);
            break;
        case 'ContextMenu':
            e.preventDefault();
            openMenu(focusTarget(el)!, menuFor(n), `Actions for ${n.name}`);
            break;
    }
});
tree.addEventListener('focusout', (e) => {
    if ((e.target as HTMLElement).matches('.tree-rename') && renaming && !rendering) finishRename(true);
});

$('sbNewBtn').addEventListener('click', (e) =>
    openMenu(
        e.currentTarget as HTMLElement,
        [
            { label: 'New document', icon: 'document', run: () => newDoc() },
            {
                label: 'New temporary document',
                icon: 'document',
                sub: 'A separate session, not saved in your library',
                run: () => share.startTemporary().catch((error) => toast(error.message)),
            },
            { label: 'New folder', icon: 'folder', run: () => newFolder() },
            'separator',
            {
                label: 'Import files…',
                icon: 'upload',
                sub: 'Markdown, HTML, or ZIP files',
                run: () => $('importInput').click(),
            },
            { label: 'Import a folder…', icon: 'folderOpen', run: () => $('importFolderInput').click() },
            {
                label: 'Restore backup…',
                icon: 'lock',
                run: () => sync.restoreRecovery().catch((error) => toast(error.message)),
            },
        ],
        'New',
    ),
);
$('sbExportAllBtn').addEventListener('click', () => io.exportZip(null));
filterInput.addEventListener('input', render);
filterInput.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && filterInput.value) {
        e.stopPropagation();
        filterInput.value = '';
        render();
    }
});

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
    if (!n || !e.dataTransfer || renaming || filterText()) {
        e.preventDefault();
        return;
    }
    dragId = n.id;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('application/x-mars-node', n.id);
    e.dataTransfer.setData('text/plain', n.name);
    rowEl(n.id)?.classList.add('dragging');
});
tree.addEventListener('dragend', () => {
    rowEl(dragId)?.classList.remove('dragging');
    dragId = null;
    clearDrop();
    endLinkGesture();
});
tree.addEventListener('dragover', (e) => {
    const files = e.dataTransfer?.types.includes('Files');
    if (!dragId && !files) return;
    const drop = dropTarget(e);
    if (dragId && !canMove(app.state.nodes, dragId, drop.parentId)) {
        clearDrop();
        return;
    }
    e.preventDefault();
    e.dataTransfer!.dropEffect = dragId ? 'move' : 'copy';
    if (lastDrop?.row !== drop.row || lastDrop?.cls !== drop.cls) {
        clearDrop();
        drop.row?.classList.add(drop.cls);
        if (!drop.row) tree.classList.add('drop-root');
        lastDrop = drop;
    }
});
tree.addEventListener('dragleave', (e) => {
    if (!tree.contains(e.relatedTarget as Node)) clearDrop();
});
tree.addEventListener('drop', async (e) => {
    e.preventDefault();
    endLinkGesture();
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
