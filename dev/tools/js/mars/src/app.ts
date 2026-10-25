// App state: the review tree (mirrored from IndexedDB), the open document, and the
// operations the sidebar, sharing, and import/export build on.
import * as db from './store/db.ts';
import { ancestors, childrenOf, descendants, moveNode, newId, nextOrder, textBytes, type TreeNode } from './tree/model.ts';
import { openThreads, splitComments } from './share/merge.ts';
import { $ } from './ui/ui.ts';
import * as viewer from './doc/viewer.js';

export const state = {
    nodes: [] as TreeNode[],
    currentId: null as string | null,
    // Folder that receives new documents: the open document's folder, or a clicked folder.
    folderId: null as string | null,
};

const listeners = new Set<() => void>();
export const onChange = (fn: () => void) => { listeners.add(fn); };
const emit = () => { for (const fn of listeners) fn(); };

export const node = (id: string | null) => (id ? state.nodes.find((n) => n.id === id) : undefined);
export const current = () => node(state.currentId);

// ponytail: a reload replaces the in-memory tree wholesale, so a write in this tab that
// races another tab's broadcast can drop out of view until the next change. Merge by
// `updatedAt` if multi-tab editing becomes common.
export async function load() {
    state.nodes = await db.allNodes();
    if (state.currentId && !current()) state.currentId = null;
    emit();
}

const now = () => Date.now();
const save = (nodes: TreeNode[], texts?: Record<string, string>) => db.putNodes(nodes, texts);

function setRoute(id: string | null) {
    const hash = id ? `#doc=${id}` : '';
    if (location.hash !== hash) history.replaceState(history.state, '', `${location.pathname}${hash}`);
}

export async function createFolder(name: string, parentId: string | null, originId?: string) {
    const n: TreeNode = { id: newId(), parentId, kind: 'folder', name, order: nextOrder(state.nodes, parentId), createdAt: now(), updatedAt: now(), open: true };
    if (originId) n.originId = originId;
    state.nodes.push(n);
    await save([n]);
    emit();
    return n;
}

// `opened`: the text comes from outside (a file, link, URL, or import) and is kept as the
// document's source text, for "changes since opened" and for re-opens.
export async function createDoc(opts: { name?: string; text?: string; parentId?: string | null; originId?: string; handle?: FileSystemFileHandle; stamp?: number; open?: boolean; opened?: boolean } = {}) {
    const id = newId();
    const parentId = opts.parentId === undefined ? state.folderId : opts.parentId;
    const text = opts.text ?? '';
    const n: TreeNode = { id, parentId, kind: 'doc', name: opts.name ?? 'Untitled', order: nextOrder(state.nodes, parentId), createdAt: now(), updatedAt: now(), originId: opts.originId ?? id, threads: openThreads(text), size: textBytes(text) };
    if (opts.handle) Object.assign(n, { handle: opts.handle, stamp: opts.stamp ?? 0, dirty: false });
    if (opts.opened) { n.opened = true; await db.putSide('opened', id, text); }
    state.nodes.push(n);
    await save([n], { [id]: text });
    expandTo(parentId);
    if (opts.open !== false) await openDoc(id);
    else emit();
    return n;
}

export const bodyOf = (text: string) => splitComments(text).body.replace(/\r\n?/g, '\n').trim();

// Records the source text as MARS last received it. The caller saves the node.
export async function setOpened(n: TreeNode, text: string) {
    n.opened = true;
    await db.putSide('opened', n.id, text);
}

// Keeps the reviewed text before an outside change replaces it, so the viewer can show what
// changed. An earlier unreviewed baseline wins, so changes add up until "Mark reviewed".
// The caller saves the node.
export async function rebase(n: TreeNode, previous: string, next: string) {
    if (n.pending || bodyOf(previous) === bodyOf(next)) return;
    await db.putSide('base', n.id, previous);
    n.pending = true;
    emit();
}

export async function markReviewed(id: string) {
    const n = node(id);
    if (!n?.pending) return;
    delete n.pending;
    await db.deleteSide('base', id);
    await save([n]);
    emit();
}

// Detaches a document from its source: no linked file, no source text, no pending review,
// and a fresh origin, as if it had been written in MARS.
export async function unlink(id: string) {
    const n = node(id);
    if (!n) return;
    for (const key of ['handle', 'stamp', 'dirty', 'pending', 'opened'] as const) delete n[key];
    n.originId = n.id;
    await Promise.all([db.deleteSide('base', id), db.deleteSide('opened', id)]);
    await save([n]);
    if (id === state.currentId) await reopen(n);
    emit();
}

// A linked file whose text differs from the stored copy, or null. Our own writes only
// refresh the stamp. `request` re-reads unconditionally and may prompt for permission.
async function readChangedFile(n: TreeNode, request = false) {
    const handle = n.handle!;
    const permission = await (request ? handle.requestPermission?.({ mode: 'read' }) : handle.queryPermission?.({ mode: 'read' }));
    if (permission !== 'granted') {
        if (request) viewer.toast('Read permission was not granted');
        return null;
    }
    const file = await handle.getFile();
    if (!request && file.lastModified === n.stamp) return null;
    const text = await file.text();
    const previous = n.id === state.currentId ? viewer.currentText() : (await db.getText(n.id)) ?? '';
    if (text === previous && !request) { n.stamp = file.lastModified; await save([n]); return null; }
    return { text, previous, stamp: file.lastModified };
}

// Reads a changed linked file into the store. Returns true when the stored text changed.
async function syncFromDisk(n: TreeNode, request = false) {
    const change = await readChangedFile(n, request);
    if (!change) return false;
    await rebase(n, change.previous, change.text);
    await setOpened(n, change.text);
    Object.assign(n, { stamp: change.stamp, dirty: false, updatedAt: now(), threads: openThreads(change.text), size: textBytes(change.text) });
    await save([n], { [n.id]: change.text });
    viewer.toast(`Reloaded ${n.handle!.name} from disk`);
    return true;
}

// Opens a stored document. A linked file that changed on disk is reloaded when the
// browser copy has no unsaved edits; otherwise the viewer flags the conflict on save.
export async function openDoc(id: string) {
    const n = node(id);
    if (!n || n.kind !== 'doc') return;
    if (state.currentId && state.currentId !== id) await viewer.flush();
    if (n.handle && !n.dirty) { try { await syncFromDisk(n); } catch {} }
    const text = (await db.getText(id)) ?? '';
    state.currentId = id;
    state.folderId = n.parentId;
    expandTo(n.parentId);
    setRoute(id);
    viewer.openDocument({ text, name: n.name, handle: n.handle ?? null, stamp: n.stamp ?? 0, dirty: !!n.dirty });
    emit();
}

// Re-reads a linked file on request (a user gesture can grant read permission).
export async function reloadFromDisk(id: string) {
    const n = node(id);
    if (!n?.handle || !(await syncFromDisk(n, true))) return;
    if (state.currentId === id) await reopen(n);
    emit();
}

async function reopen(n: TreeNode) {
    viewer.openDocument({ text: (await db.getText(n.id)) ?? '', name: n.name, handle: n.handle ?? null, stamp: n.stamp ?? 0, dirty: !!n.dirty });
}

// Linked files exist only where the File System Access pickers do (Chromium); Firefox and
// Safari have a negative position on them, so there the option and the poll stay inert.
// ponytail: polls the open linked file's timestamp every 2 s while the tab is visible.
// Switch to FileSystemObserver once it is Baseline.
const LIVE_KEY = 'mars.liveFiles';
let live = false;
try { live = localStorage.getItem(LIVE_KEY) === '1'; } catch {}
if ('showOpenFilePicker' in window) {
    const input = $<HTMLInputElement>('optLiveFiles');
    $('optLiveFilesWrap').hidden = false;
    input.checked = live;
    input.addEventListener('change', () => {
        live = input.checked;
        try { localStorage.setItem(LIVE_KEY, live ? '1' : ''); } catch {}
    });
}
const notified = new Map<string, number>(); // doc id -> file stamp already announced
let polling = false;
setInterval(async () => {
    const n = current();
    if (!n?.handle || document.hidden || polling) return;
    polling = true;
    try {
        await viewer.flush();
        if (n.dirty || n !== current()) return;
        if (live) {
            if (await syncFromDisk(n)) { await reopen(n); emit(); }
            return;
        }
        const change = await readChangedFile(n);
        if (!change || notified.get(n.id) === change.stamp) return;
        notified.set(n.id, change.stamp);
        viewer.toast(`${n.handle.name} changed on disk`, { label: 'Reload', run: () => reloadFromDisk(n.id) });
    } catch {} finally { polling = false; }
}, 2000);

export function closeDoc() {
    state.currentId = null;
    setRoute(null);
    viewer.closeDocument();
    emit();
}

// The viewer is about to load new content (Open, URL, example) over a non-empty document:
// the next persist creates a new tree entry instead of overwriting this one.
export function detach() {
    state.currentId = null;
    setRoute(null);
    emit();
}

type PersistedDoc = { text: string; name: string; handle: FileSystemFileHandle | null; stamp: number; dirty: boolean };
let persistQueue: Promise<void> = Promise.resolve();
// Serialized, so two quick saves of an unsaved document create one tree entry.
export function persist(doc: PersistedDoc) {
    const run = persistQueue.then(() => persistOne(doc));
    persistQueue = run.catch(() => {});
    return run;
}

async function persistOne(doc: PersistedDoc) {
    let n = current();
    if (!n) {
        await createDoc({ name: doc.name, text: doc.text, handle: doc.handle ?? undefined, stamp: doc.stamp, open: false });
        n = state.nodes.at(-1)!;
        state.currentId = n.id;
        setRoute(n.id);
        emit();
        return;
    }
    Object.assign(n, { name: doc.name, updatedAt: now(), threads: openThreads(doc.text), size: textBytes(doc.text), stamp: doc.stamp, dirty: doc.dirty });
    if (doc.handle) n.handle = doc.handle; else delete n.handle;
    await save([n], { [n.id]: doc.text });
    emit();
}

export async function rename(id: string, name: string) {
    const n = node(id);
    if (!n || !name.trim() || n.name === name) return;
    n.name = name.trim();
    n.updatedAt = now();
    await save([n]);
    if (id === state.currentId) viewer.renameDocument(n.name);
    emit();
}

export async function move(id: string, parentId: string | null, beforeId: string | null = null) {
    const changed = moveNode(state.nodes, id, parentId, beforeId);
    if (!changed.length) return false;
    await save(changed);
    expandTo(parentId);
    if (id === state.currentId) state.folderId = parentId;
    emit();
    return true;
}

export async function setOpen(id: string, open: boolean) {
    const n = node(id);
    if (!n || n.kind !== 'folder' || !!n.open === open) return;
    n.open = open;
    emit();
    await save([n]);
}

function expandTo(folderId: string | null) {
    const chain = [...ancestors(state.nodes, folderId), node(folderId)].filter((n): n is TreeNode => !!n && !n.open);
    for (const n of chain) n.open = true;
    if (chain.length) save(chain);
}

export async function duplicate(id: string) {
    const n = node(id);
    if (n?.kind !== 'doc') return;
    const text = (await db.getText(id)) ?? '';
    const copy = await createDoc({ name: `${n.name} (copy)`, text, parentId: n.parentId, open: false });
    await move(copy.id, n.parentId, childrenOf(state.nodes, n.parentId)[childrenOf(state.nodes, n.parentId).indexOf(n) + 1]?.id ?? null);
    await openDoc(copy.id);
}

// Deletes a node and its subtree, returning an undo that restores both nodes and texts.
export async function remove(id: string) {
    const n = node(id);
    if (!n) return null;
    const gone = [n, ...descendants(state.nodes, id)];
    const texts: Record<string, string> = {};
    for (const d of gone) if (d.kind === 'doc') texts[d.id] = (await db.getText(d.id)) ?? '';
    const ids = new Set(gone.map((d) => d.id));
    const wasOpen = state.currentId && ids.has(state.currentId) ? state.currentId : null;
    // Leave the tree first, so the smart-search indexer stops writing entries for these documents.
    state.nodes = state.nodes.filter((d) => !ids.has(d.id));
    await db.deleteNodes([...ids]);
    if (state.folderId && ids.has(state.folderId)) state.folderId = n.parentId;
    if (wasOpen) closeDoc(); else emit();
    return async () => {
        state.nodes.push(...gone);
        await save(gone, texts);
        if (wasOpen) await openDoc(wasOpen); else emit();
    };
}

export const docText = async (id: string) => (id === state.currentId ? viewer.currentText() : (await db.getText(id)) ?? '');

export async function findOrCreateFolder(name: string, parentId: string | null) {
    return state.nodes.find((n) => n.kind === 'folder' && n.parentId === parentId && n.name === name) ?? createFolder(name, parentId);
}

export function uniqueName(name: string, parentId: string | null) {
    const taken = new Set(childrenOf(state.nodes, parentId).map((n) => n.name));
    let out = name, i = 2;
    while (taken.has(out)) out = `${name} (${i++})`;
    return out;
}
