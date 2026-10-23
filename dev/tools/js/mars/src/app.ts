// App state: the review tree (mirrored from IndexedDB), the open document, and the
// operations the sidebar, sharing, and import/export build on.
import * as db from './store/db.ts';
import { ancestors, childrenOf, descendants, moveNode, newId, nextOrder, textBytes, type TreeNode } from './tree/model.ts';
import { openThreads } from './share/merge.ts';
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

export async function createDoc(opts: { name?: string; text?: string; parentId?: string | null; originId?: string; handle?: FileSystemFileHandle; stamp?: number; open?: boolean } = {}) {
    const id = newId();
    const parentId = opts.parentId === undefined ? state.folderId : opts.parentId;
    const text = opts.text ?? '';
    const n: TreeNode = { id, parentId, kind: 'doc', name: opts.name ?? 'Untitled', order: nextOrder(state.nodes, parentId), createdAt: now(), updatedAt: now(), originId: opts.originId ?? id, threads: openThreads(text), size: textBytes(text) };
    if (opts.handle) Object.assign(n, { handle: opts.handle, stamp: opts.stamp ?? 0, dirty: false });
    state.nodes.push(n);
    await save([n], { [id]: text });
    expandTo(parentId);
    if (opts.open !== false) await openDoc(id);
    else emit();
    return n;
}

// Opens a stored document. A linked file that changed on disk is reloaded when the
// browser copy has no unsaved edits; otherwise the viewer flags the conflict on save.
export async function openDoc(id: string) {
    const n = node(id);
    if (!n || n.kind !== 'doc') return;
    if (state.currentId && state.currentId !== id) await viewer.flush();
    let text = (await db.getText(id)) ?? '';
    if (n.handle && !n.dirty) {
        try {
            if ((await n.handle.queryPermission?.({ mode: 'read' })) === 'granted') {
                const file = await n.handle.getFile();
                if (file.lastModified !== n.stamp) {
                    text = await file.text();
                    Object.assign(n, { stamp: file.lastModified, updatedAt: now(), threads: openThreads(text), size: textBytes(text) });
                    await save([n], { [id]: text });
                    viewer.toast(`Reloaded ${n.handle.name} from disk`);
                }
            }
        } catch {}
    }
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
    if (!n?.handle) return;
    if ((await n.handle.requestPermission?.({ mode: 'read' })) !== 'granted') { viewer.toast('Read permission was not granted'); return; }
    const file = await n.handle.getFile();
    const text = await file.text();
    Object.assign(n, { stamp: file.lastModified, dirty: false, updatedAt: now(), threads: openThreads(text), size: textBytes(text) });
    await save([n], { [id]: text });
    if (state.currentId === id) viewer.openDocument({ text, name: n.name, handle: n.handle, stamp: n.stamp, dirty: false });
    viewer.toast(`Reloaded ${n.handle.name} from disk`);
    emit();
}

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
    await db.deleteNodes([...ids]);
    state.nodes = state.nodes.filter((d) => !ids.has(d.id));
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
