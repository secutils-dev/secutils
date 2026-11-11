// App state: the review tree (mirrored from the active store), the open document, and the
// operations the sidebar, sharing, and import/export build on.
//
// Owns `state`, the document route in the address bar, the linked-file poll, and the persistence queue.
// Does not own storage transactions (`store/db.ts`), rendering (`doc/viewer.js`), or connection
// protocol work (`sync/`).
// Invariants:
// - `state.nodes` is the in-memory copy of the stored tree. An operation changes it, saves the
//   changed nodes through `db`, and notifies the listeners.
// - Saves of the open document run one at a time through `persistQueue`.
// - An asynchronous open checks its `InputOperation` after each wait and before it selects a document.
import * as db from './store/db.ts';
import {
    ancestors,
    childrenOf,
    descendants,
    moveNode,
    newId,
    nextOrder,
    textBytes,
    type TreeNode,
} from './tree/model.ts';
import { openThreads, splitComments } from './share/merge.ts';
import { $ } from './ui/ui.ts';
import * as viewer from './doc/viewer.js';
import { beginInput, cancelInput, checkInput, type InputOperation } from './io/input.ts';
import { validWidth, type ReadingWidth } from './export/settings.ts';
import { TemporaryRoute, type TemporaryLinkStatus } from './share/temporary.ts';
import * as sync from './sync/index.ts';
import { DEFAULT_DESTINATION_KEY, incomingFolder } from './io/destination.ts';

/**
 * The single mutable app state. Read it freely. Change it through the functions of this module,
 * or call `notify()` after a direct change.
 */
export const state = {
    // Every folder and document, as a flat list. `load()` replaces the array and its node objects.
    nodes: [] as TreeNode[],
    // The open document, or null for the empty state.
    currentId: null as string | null,
    // Folder that receives new documents: the open document's folder, or a clicked folder.
    folderId: null as string | null,
    // The folder that receives files, links, and URLs, or null when none is set or it is not editable.
    defaultIncomingFolder: null as string | null,
    // State of the address-bar copy of the current temporary document (see `TemporaryLinkStatus`).
    temporaryLink: 'empty' as TemporaryLinkStatus,
    // The message of the storage failure that forced a temporary session, or ''.
    storageError: '' as string,
};

const listeners = new Set<() => void>();
/** Registers a listener that runs synchronously after each state change. There is no removal. */
export const onChange = (fn: () => void) => {
    listeners.add(fn);
};
const emit = () => {
    for (const fn of listeners) fn();
};
/** Notifies the listeners. For a module that changes `state` or a node in place, such as Sync. */
export const notify = emit;

/** The node with this id, or undefined for null and for an unknown id. */
export const node = (id: string | null) => (id ? state.nodes.find((n) => n.id === id) : undefined);
/** The node of the open document, or undefined. */
export const current = () => node(state.currentId);

/**
 * Reads the tree and the default folder from storage into `state`, then notifies.
 * It replaces every node object, so a node reference from before the call is stale.
 * It clears `currentId` and `folderId` when their node is gone. Except for `ended`, a connection
 * phase comes from the transport state of this tab, not from the stored node.
 * Rejects when storage fails.
 */
export async function load() {
    const [nodes, destination] = await Promise.all([db.allNodes(), db.getMeta<string>(DEFAULT_DESTINATION_KEY)]);
    state.nodes = sync.connectionStates.restore(nodes);
    state.defaultIncomingFolder = incomingFolder(nodes, destination)?.id ?? null;
    if (state.currentId && !current()) state.currentId = null;
    if (state.folderId && !node(state.folderId)) state.folderId = null;
    emit();
}
/** The default folder for incoming documents, or undefined when it is unset, deleted, or not editable. */
export const defaultIncomingFolder = () => incomingFolder(state.nodes, state.defaultIncomingFolder);
/**
 * Saves the default folder for incoming documents and tells other tabs to reload. Null clears it.
 * Throws in a temporary session, and for a folder that is missing or not editable.
 */
export async function setDefaultIncomingFolder(id: string | null) {
    if (db.isTemporaryStorage() || (id && !incomingFolder(state.nodes, id)))
        throw new Error('Choose an editable library folder.');
    await db.setMeta(DEFAULT_DESTINATION_KEY, id, true);
    state.defaultIncomingFolder = id;
    emit();
}

const now = () => Date.now();
const save = (nodes: TreeNode[], texts?: Record<string, string>) => db.putNodes(nodes, texts);
// The address-bar copy of the current temporary document. `replaceState` adds no history entry
// and fires no `hashchange`, so the write does not open the document again.
const temporaryRoute = new TemporaryRoute(
    (hash) => history.replaceState(history.state, '', `${location.pathname}${hash}`),
    (status) => {
        state.temporaryLink = status;
        emit();
    },
);
/** Drops a pending address-bar update, so a late encode cannot overwrite an incoming link. */
export const cancelTemporaryRoute = () => temporaryRoute.cancel();
/**
 * Moves this tab to a temporary session after a storage failure. It empties the in-memory tree
 * and the selection, records the error for the banner, and shows a toast. The stored library
 * does not change.
 */
export function useTemporaryFallback(error: unknown) {
    db.useTemporaryStorage();
    state.storageError = (error as Error)?.message ?? String(error);
    state.nodes = [];
    state.currentId = state.folderId = state.defaultIncomingFolder = null;
    viewer.toast('Browser storage is unavailable. The document is now temporary.');
    emit();
}

/**
 * Writes the route of the open document with `history.replaceState`.
 * Library session: `#doc=<id>`, or no fragment. Temporary session: the complete document in the
 * fragment (see `TemporaryRoute`). A live document leaves the address unchanged.
 */
async function setRoute(id: string | null) {
    if (db.isTemporaryStorage()) {
        const n = node(id);
        if (n?.connection?.kind === 'live') {
            temporaryRoute.cancel();
            return;
        }
        await temporaryRoute.update(
            n?.kind === 'doc'
                ? {
                      path: [],
                      name: n.name,
                      originId: n.originId ?? n.id,
                      text: (await db.getText(n.id)) ?? '',
                      readingWidth: n.readingWidth ?? viewer.exportReadingWidth(),
                  }
                : null,
        );
        return;
    }
    temporaryRoute.cancel();
    const hash = id ? `#doc=${id}` : '';
    if (location.hash !== hash) history.replaceState(history.state, '', `${location.pathname}${hash}`);
}

/**
 * Creates an open folder as the last child of `parentId`, saves it, and notifies.
 * `originId` marks a folder that came from a shared bundle. A folder in a connected parent joins
 * the collection in the background. A failure there shows a toast, and the local folder stays.
 */
export async function createFolder(name: string, parentId: string | null, originId?: string) {
    const n: TreeNode = {
        id: newId(),
        parentId,
        kind: 'folder',
        name,
        order: nextOrder(state.nodes, parentId),
        createdAt: now(),
        updatedAt: now(),
        open: true,
    };
    if (originId) n.originId = originId;
    state.nodes.push(n);
    await save([n]);
    if (node(parentId)?.connection)
        void sync.created(n.id).catch((error) => viewer.toast(`Could not enroll the new folder: ${error.message}`));
    emit();
    return n;
}

/**
 * Creates a document, saves the node and its text in one transaction, and opens it unless
 * `open` is false.
 *
 * - `parentId`: undefined selects `state.folderId`. Null selects the top level.
 * - `originId`: defaults to the new id.
 * - `opened`: the text comes from outside (a file, link, URL, or import) and is kept as the
 *   document's source text, for "changes since opened" and for re-opens. This side record is
 *   written first, in its own transaction.
 * - `handle`, `stamp`: the linked file and its `lastModified` at the read.
 * - `input`: the operation that supplied the text. A superseded operation throws `AbortError`.
 *   A save that started before the check still completes, but the document does not open.
 *
 * A storage failure in a library session moves the tab to a temporary session and creates the
 * document there. Other errors propagate. A document in a connected folder joins the
 * collection in the background.
 */
export async function createDoc(
    opts: {
        name?: string;
        text?: string;
        parentId?: string | null;
        originId?: string;
        handle?: FileSystemFileHandle;
        stamp?: number;
        open?: boolean;
        opened?: boolean;
        input?: InputOperation;
        readingWidth?: ReadingWidth;
    } = {},
) {
    // A direct open, with no input operation, supersedes a pending asynchronous input.
    if (!opts.input && opts.open !== false) cancelInput();
    checkInput(opts.input);
    const id = newId();
    const parentId = opts.parentId === undefined ? state.folderId : opts.parentId;
    const text = opts.text ?? '';
    const n: TreeNode = {
        id,
        parentId,
        kind: 'doc',
        name: opts.name ?? 'Untitled',
        order: nextOrder(state.nodes, parentId),
        createdAt: now(),
        updatedAt: now(),
        originId: opts.originId ?? id,
        threads: openThreads(text),
        size: textBytes(text),
    };
    if (validWidth(opts.readingWidth)) n.readingWidth = opts.readingWidth;
    if (opts.handle) Object.assign(n, { handle: opts.handle, stamp: opts.stamp ?? 0, dirty: false });
    try {
        if (opts.opened) {
            n.opened = true;
            await db.putSide('opened', id, text);
        }
        checkInput(opts.input);
        state.nodes.push(n);
        await save([n], { [id]: text });
    } catch (error) {
        checkInput(opts.input);
        // Only a storage failure of the library falls back. The retry runs in the temporary session:
        // its tree is empty, and it keeps no file handle.
        if (db.isTemporaryStorage() || !db.isStorageFailure(error)) throw error;
        useTemporaryFallback(error);
        return createDoc({ ...opts, parentId: null, originId: n.originId, handle: undefined });
    }
    checkInput(opts.input);
    expandTo(parentId);
    if (opts.open !== false) await openDoc(id, opts.input);
    else emit();
    if (node(parentId)?.connection)
        void sync
            .created(n.id)
            .catch((error) =>
                viewer.toast(`Could not enroll the new document: ${error.message}. Its local copy is retained.`),
            );
    return n;
}

/**
 * The text that decides whether two copies differ: the body without the review block, with
 * `\n` line endings and no outer whitespace.
 */
export const bodyOf = (text: string) => splitComments(text).body.replace(/\r\n?/g, '\n').trim();

/**
 * Records the source text as MARS last received it (the `opened` side record) and sets
 * `n.opened`. The caller saves the node.
 */
export async function setOpened(n: TreeNode, text: string) {
    n.opened = true;
    await db.putSide('opened', n.id, text);
}

/**
 * Keeps the reviewed text before an outside change replaces it, so the viewer can show what
 * changed. An earlier unreviewed baseline wins, so changes add up until "Mark reviewed".
 * It does nothing when the two bodies are equal. It writes the `base` side record, sets
 * `n.pending`, and notifies. The caller saves the node.
 */
export async function rebase(n: TreeNode, previous: string, next: string) {
    if (n.pending || bodyOf(previous) === bodyOf(next)) return;
    await db.putSide('base', n.id, previous);
    n.pending = true;
    emit();
}

/**
 * Ends the review of an outside change: deletes the `base` side record, clears `pending`, and
 * saves the node. A connected document reports the review to Sync first. No effect when
 * nothing is pending.
 */
export async function markReviewed(id: string) {
    const n = node(id);
    if (!n?.pending) return;
    if (n.connection) await sync.markReviewed(id);
    delete n.pending;
    await db.deleteSide('base', id);
    await save([n]);
    emit();
}

/**
 * Detaches a document from its source: no linked file, no source text, no pending review,
 * and a fresh origin, as if it had been written in MARS. The open document reloads in the viewer.
 */
export async function unlink(id: string) {
    const n = node(id);
    if (!n) return;
    for (const key of ['handle', 'stamp', 'dirty', 'pending', 'opened'] as const) delete n[key];
    n.originId = newId();
    await Promise.all([db.deleteSide('base', id), db.deleteSide('opened', id)]);
    await save([n]);
    if (id === state.currentId) await reopen(n);
    emit();
}

/**
 * Returns a linked file whose text differs from the stored copy, or null. Our own writes only
 * refresh the stamp. `request` re-reads unconditionally and may prompt for permission.
 * Returns null when read permission is not granted and, without `request`, when `lastModified`
 * equals the stamp.
 * The comparison uses the viewer text for the open document, so unsaved edits count.
 */
async function readChangedFile(n: TreeNode, request = false) {
    const handle = n.handle!;
    const permission = await (request
        ? handle.requestPermission?.({ mode: 'read' })
        : handle.queryPermission?.({ mode: 'read' }));
    if (permission !== 'granted') {
        if (request) viewer.toast('Read permission was not granted');
        return null;
    }
    const file = await handle.getFile();
    if (!request && file.lastModified === n.stamp) return null;
    const text = await file.text();
    const previous = n.id === state.currentId ? viewer.currentText() : ((await db.getText(n.id)) ?? '');
    if (text === previous && !request) {
        n.stamp = file.lastModified;
        await save([n]);
        return null;
    }
    return { text, previous, stamp: file.lastModified };
}

/**
 * Reads a changed linked file into the store. Returns true when the stored text changed.
 * The previous text becomes the review baseline, and the file text becomes the `opened` record.
 */
async function syncFromDisk(n: TreeNode, request = false) {
    const change = await readChangedFile(n, request);
    if (!change) return false;
    await rebase(n, change.previous, change.text);
    await setOpened(n, change.text);
    Object.assign(n, {
        stamp: change.stamp,
        dirty: false,
        updatedAt: now(),
        threads: openThreads(change.text),
        size: textBytes(change.text),
    });
    await save([n], { [n.id]: change.text });
    viewer.toast(`Reloaded ${n.handle!.name} from disk`);
    return true;
}

/**
 * Opens a stored document. A linked file that changed on disk is reloaded when the
 * browser copy has no unsaved edits; otherwise the viewer flags the conflict on save.
 *
 * Without `input`, the call starts an input operation, which cancels another pending open.
 * It returns without effect when the id is not a document or the operation is superseded
 * during a wait. A connection draft conflict in the viewer also stops it, with a toast.
 * Side effects: saves pending edits of the previous document, sets `currentId` and `folderId`,
 * expands the ancestors, and writes the route. A connected document also notifies Sync.
 */
export async function openDoc(id: string, input?: InputOperation) {
    const n = node(id);
    if (!n || n.kind !== 'doc') return;
    const operation = input ?? beginInput();
    if (!operation.current()) return;
    // The viewer still holds the previous document. Save its pending edits before the switch.
    if (state.currentId && state.currentId !== id) await viewer.flush();
    if (viewer.hasConnectionDraftConflict()) {
        viewer.toast('Save or download the draft before switching documents.');
        return;
    }
    if (!operation.current()) return;
    // A document with unsaved browser edits and a connected document do not reload from disk.
    // A read failure keeps the stored text.
    if (n.handle && !n.dirty && !n.connection) {
        try {
            await syncFromDisk(n);
        } catch {}
    }
    if (!operation.current()) return;
    const text = (await db.getText(id)) ?? '';
    // The node can disappear during the waits above.
    if (!operation.current() || !node(id)) return;
    state.currentId = id;
    state.folderId = n.parentId;
    expandTo(n.parentId);
    await setRoute(id);
    viewer.openDocument({
        text,
        name: n.name,
        handle: n.handle ?? null,
        stamp: n.stamp ?? 0,
        dirty: !!n.dirty,
        readingWidth: n.localReadingWidth ?? n.readingWidth,
    });
    if (n.connection) {
        // A connected document writes its linked file only on an explicit Save.
        n.manualFileSave = true;
        await sync.present(id);
        void sync.opened(id);
    }
    emit();
}

/**
 * Re-reads a linked file on request (a user gesture can grant read permission), and reloads
 * the viewer when the document is open.
 */
export async function reloadFromDisk(id: string) {
    const n = node(id);
    if (!n?.handle || !(await syncFromDisk(n, true))) return;
    if (state.currentId === id) await reopen(n);
    emit();
}

/** Loads the stored text into the viewer again, when `n` is still the open document. */
async function reopen(n: TreeNode) {
    const text = (await db.getText(n.id)) ?? '';
    if (state.currentId !== n.id || !node(n.id)) return;
    viewer.openDocument({
        text,
        name: n.name,
        handle: n.handle ?? null,
        stamp: n.stamp ?? 0,
        dirty: !!n.dirty,
        readingWidth: n.localReadingWidth ?? n.readingWidth,
    });
}

// Linked files exist only where the File System Access pickers do (Chromium); Firefox and
// Safari have a negative position on them, so there the option and the poll stay inert.
// ponytail: polls the open linked file's timestamp every 2 s while the tab is visible.
// Switch to FileSystemObserver once it is Baseline.
/** `localStorage` key of "Reload linked files automatically": '1' is on. */
const LIVE_KEY = 'mars.liveFiles';
let live = false;
try {
    live = localStorage.getItem(LIVE_KEY) === '1';
} catch {}
if ('showOpenFilePicker' in window) {
    const input = $<HTMLInputElement>('optLiveFiles');
    $('optLiveFilesWrap').hidden = false;
    input.checked = live;
    input.addEventListener('change', () => {
        live = input.checked;
        try {
            localStorage.setItem(LIVE_KEY, live ? '1' : '');
        } catch {}
    });
}
const notified = new Map<string, number>(); // doc id -> file stamp already announced
// One poll at a time: a slow file read must not overlap the next tick.
let polling = false;
setInterval(async () => {
    const n = current();
    // `flush()` commits an open block or cell editor, so the poll waits until the reader finishes the edit.
    if (!n?.handle || n.connection || document.hidden || polling || viewer.isEditing()) return;
    polling = true;
    try {
        await viewer.flush();
        // Browser edits are newer than the file, or the reader switched documents during the flush.
        if (n.dirty || n !== current()) return;
        if (live) {
            if (await syncFromDisk(n)) {
                await reopen(n);
                emit();
            }
            return;
        }
        const change = await readChangedFile(n);
        if (!change || notified.get(n.id) === change.stamp) return;
        notified.set(n.id, change.stamp);
        viewer.toast(`${n.handle.name} changed on disk`, { label: 'Reload', run: () => reloadFromDisk(n.id) });
    } catch {
    } finally {
        polling = false;
    }
}, 2000);

/** Closes the open document: cancels pending input, clears the route, and empties the viewer. */
export function closeDoc() {
    cancelInput();
    state.currentId = null;
    setRoute(null);
    viewer.closeDocument();
    emit();
}

/**
 * The viewer is about to load new content (Open, URL, example) over a non-empty document:
 * the next persist creates a new tree entry instead of overwriting this one.
 */
export function detach() {
    state.currentId = null;
    setRoute(null);
    emit();
}

/**
 * Stores the reading width that the reader chose for the open document. An invalid width is ignored.
 * A connected document stores it as `localReadingWidth`, a display preference of this device.
 * Another document stores `readingWidth`, which travels in links and in the temporary route.
 */
export async function setReadingWidth(width: ReadingWidth) {
    const n = current();
    if (!n || !validWidth(width)) return;
    if (n.connection) {
        n.localReadingWidth = width;
        await save([n]);
        return;
    }
    if (n.readingWidth === width) return;
    n.readingWidth = width;
    if (n.connection) await sync.metadata(n.id);
    else await save([n]);
    if (db.isTemporaryStorage()) await setRoute(n.id);
}

// Documents whose stale-tab warning was shown. An applied change clears the entry.
const staleNotified = new Set<string>();
/**
 * Another tab saved the open document. Without this step, the next save of this tab writes its
 * stale text over that change. A connected document gets its updates from `sync.refresh`.
 *
 * Call it after `load()`, with the ids from the broadcast. It applies the stored text to the
 * viewer. When the viewer holds an unsaved edit, it keeps the edit and warns the reader once.
 */
export async function adoptRemoteChange(ids: string[]) {
    const n = current();
    if (!n || n.kind !== 'doc' || n.connection || !ids.includes(n.id) || db.isTemporaryStorage()) return;
    const text = (await db.getText(n.id)) ?? '';
    if (n !== current() || (text === viewer.currentText() && n.name === viewer.currentName())) return;
    if (viewer.applyConnectionProjection({ text, name: n.name })) {
        staleNotified.delete(n.id);
        return;
    }
    // This tab has an unsaved edit or an open editor. Keep it, and tell the reader once.
    if (staleNotified.has(n.id)) return;
    staleNotified.add(n.id);
    viewer.toast(
        'This document changed in another tab. Your next save here replaces that change. Reload this tab to get it.',
    );
}

/** The viewer state of one document, as `persist` receives it. */
type PersistedDoc = { text: string; name: string; handle: FileSystemFileHandle | null; stamp: number; dirty: boolean };
// Tail of the serialized chain. It never rejects, so one failed task does not block the next.
let persistQueue: Promise<void> = Promise.resolve();
/**
 * Runs `run` after every queued save and before every later one, and returns its result.
 * A rejection reaches the caller only. Use it for tree work that must not interleave with a
 * save of the open document. Examples: a reload after another tab's write, and a Sync publication.
 */
export function serializePersistence<T>(run: () => Promise<T>) {
    const task = persistQueue.then(run);
    persistQueue = task.then(
        () => {},
        () => {},
    );
    return task;
}
/**
 * Saves the viewer state of the open document. Serialized, so two quick saves of an unsaved
 * document create one tree entry.
 *
 * A storage failure of a local library document moves the tab to a temporary session and saves
 * the document there. Every other failure rejects, and the queue continues.
 */
export function persist(doc: PersistedDoc) {
    const run = persistQueue
        .then(() => persistOne(doc))
        .catch(async (error) => {
            if (current()?.connection || db.isTemporaryStorage() || !db.isStorageFailure(error)) throw error;
            // Read these before the fallback empties the tree.
            const originId = current()?.originId,
                readingWidth = viewer.exportReadingWidth();
            useTemporaryFallback(error);
            const n = await createDoc({ name: doc.name, text: doc.text, originId, readingWidth, open: false });
            state.currentId = n.id;
            await setRoute(n.id);
            emit();
        });
    persistQueue = run.catch(() => {});
    return run;
}

/**
 * One queued save. Without an open document it creates the tree entry and selects it. Otherwise
 * it updates the current node. A connected document saves through `sync.persist`, not through `db`.
 */
async function persistOne(doc: PersistedDoc) {
    let n = current();
    if (!n) {
        // Use the returned node: an import or a duplicate can push another node while this save waits.
        n = await createDoc({
            name: doc.name,
            text: doc.text,
            handle: doc.handle ?? undefined,
            stamp: doc.stamp,
            open: false,
        });
        state.currentId = n.id;
        await setRoute(n.id);
        emit();
        return;
    }
    Object.assign(n, {
        name: doc.name,
        updatedAt: now(),
        threads: openThreads(doc.text),
        size: textBytes(doc.text),
        stamp: doc.stamp,
        dirty: doc.dirty,
    });
    if (doc.handle) n.handle = doc.handle;
    else delete n.handle;
    if (n.connection) {
        if (!['ended', 'attention'].includes(n.connection.phase ?? '')) n.connection.phase = 'waiting';
        await sync.persist(n, doc.text);
        emit();
        return;
    }
    await save([n], { [n.id]: doc.text });
    if (db.isTemporaryStorage()) await setRoute(n.id);
    emit();
}

/**
 * Renames a node. A blank or unchanged name has no effect. A connected node publishes the name
 * through Sync. The open document updates in the viewer and in the temporary route.
 */
export async function rename(id: string, name: string) {
    const n = node(id);
    if (!n || !name.trim() || n.name === name) return;
    n.name = name.trim();
    n.updatedAt = now();
    if (n.connection) await sync.metadata(id);
    else await save([n]);
    if (id === state.currentId) viewer.renameDocument(n.name);
    if (db.isTemporaryStorage() && id === state.currentId) await setRoute(id);
    emit();
}

/**
 * Moves a node under `parentId`, before `beforeId` or as the last child.
 * Returns false when Sync refuses the move, when the target is the node or its descendant, and
 * when nothing changes. A connected node publishes its position. A node that enters a connected
 * folder joins the collection.
 */
export async function move(id: string, parentId: string | null, beforeId: string | null = null) {
    if ((node(id)?.connection || node(parentId)?.connection) && !(await sync.beforeMove(id, parentId))) return false;
    const changed = moveNode(state.nodes, id, parentId, beforeId);
    if (!changed.length) return false;
    await save(changed);
    if (node(id)?.connection) await sync.metadata(id);
    else if (node(parentId)?.connection) await sync.created(id);
    expandTo(parentId);
    if (id === state.currentId) state.folderId = parentId;
    emit();
    return true;
}

/**
 * Expands or collapses a folder. The tree updates before the save completes.
 * Each expand of a connected folder reports the open to Sync, also when the folder is open already.
 */
export async function setOpen(id: string, open: boolean) {
    const n = node(id);
    if (!n || n.kind !== 'folder') return;
    if (!!n.open === open) {
        if (open && n.connection) void sync.opened(id).catch(() => {});
        return;
    }
    n.open = open;
    emit();
    await save([n]);
    if (open && n.connection) void sync.opened(id).catch((error) => viewer.toast(error.message));
}

/** Opens `folderId` and its closed ancestors, and saves them without a wait. The caller notifies. */
function expandTo(folderId: string | null) {
    const chain = [...ancestors(state.nodes, folderId), node(folderId)].filter((n): n is TreeNode => !!n && !n.open);
    for (const n of chain) n.open = true;
    if (chain.length) save(chain);
}

/**
 * Copies a document with its review threads, places the copy after the original, and opens it.
 * The copy has its own origin, so a link to the original does not merge into it.
 */
export async function duplicate(id: string) {
    const n = node(id);
    if (n?.kind !== 'doc') return;
    const text = (await db.getText(id)) ?? '';
    const copy = await createDoc({
        name: `${n.name} (copy)`,
        text,
        parentId: n.parentId,
        open: false,
        readingWidth: n.readingWidth,
    });
    await move(
        copy.id,
        n.parentId,
        childrenOf(state.nodes, n.parentId)[childrenOf(state.nodes, n.parentId).indexOf(n) + 1]?.id ?? null,
    );
    await openDoc(copy.id);
}

/**
 * Deletes a node and its subtree, returning an undo that restores both nodes and texts.
 * Returns null for an unknown id. A storage failure restores the in-memory tree and rethrows.
 * The undo does not restore side records (`base`, `opened`, `vec`, `sync`).
 */
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
    try {
        await db.deleteNodes([...ids]);
    } catch (error) {
        state.nodes.push(...gone.filter((node) => !state.nodes.some((current) => current.id === node.id)));
        emit();
        throw error;
    }
    if (state.folderId && ids.has(state.folderId)) state.folderId = n.parentId;
    if (wasOpen) closeDoc();
    else emit();
    return async () => {
        state.nodes.push(...gone);
        await save(gone, texts);
        if (wasOpen) await openDoc(wasOpen);
        else emit();
    };
}

/** The current text of a document: the viewer text for the open one, the stored text otherwise. */
export const docText = async (id: string) =>
    id === state.currentId ? viewer.currentText() : ((await db.getText(id)) ?? '');

/** The child folder of `parentId` with exactly this name. Creates it when there is none. */
export async function findOrCreateFolder(name: string, parentId: string | null) {
    return (
        state.nodes.find((n) => n.kind === 'folder' && n.parentId === parentId && n.name === name) ??
        createFolder(name, parentId)
    );
}

/** `name`, or `name (2)`, `name (3)`, and so on: the first form that no child of `parentId` uses. */
export function uniqueName(name: string, parentId: string | null) {
    const taken = new Set(childrenOf(state.nodes, parentId).map((n) => n.name));
    let out = name,
        i = 2;
    while (taken.has(out)) out = `${name} (${i++})`;
    return out;
}
