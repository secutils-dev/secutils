// Review storage in IndexedDB: `nodes` (the folder/document tree), `docs` (document text,
// kept apart so listing the tree never loads bodies), and `meta` (small app settings).
//
// Owns the database connection, the memory backend of a temporary session, the transaction
// helper, and the change broadcast to other tabs. It does not own the meaning of Sync records:
// the connected-content functions take prepared data from `sync/` and store it.
// Invariants:
// - Every function works on the active backend. While the memory backend is active, no call
//   reads or writes IndexedDB, and nothing is broadcast. `promoteConnected` is the one exit.
// - One call is one IndexedDB transaction: its writes commit together or not at all.
// - Network and cryptographic work finishes before a transaction opens. A transaction with
//   no pending request commits by itself.
// - A compare-and-set function throws `ReplicaConflict`, and writes nothing, when the stored
//   state differs from the state that the caller prepared against.
// - The database opens without a version and keeps unknown stores. It is never downgraded or deleted.
// - A node write in the library broadcasts the ids of the changed nodes to other tabs.
import { descendants, textBytes, type TreeNode } from '../tree/model.ts';
import { openThreads } from '../share/merge.ts';
import { MemoryStore } from './memory.ts';
import { syncDebug, type ConflictReason } from '../sync/debug.ts';
import type { SessionRecord } from '../sync/session.ts';

const DB_NAME = 'mars';
// The shared connection. Null before the first use, after a `versionchange`, and after a
// switch to the memory backend.
let dbPromise: Promise<IDBDatabase> | null = null;
const memory = new MemoryStore();
const address = new URL(location.href);
// True while the memory backend is active. The page address decides it before the first
// storage call: `?temporary=1`, `t=1` in the fragment, or a `live` fragment.
let temporary =
    address.searchParams.get('temporary') === '1' ||
    new URLSearchParams(address.hash.slice(1)).get('t') === '1' ||
    new URLSearchParams(address.hash.slice(1)).has('live');
/** True while this tab stores documents in memory and not in the library. */
export const isTemporaryStorage = () => temporary;
// Counts backend switches in this tab.
let storageRevision = 0;
/**
 * A counter that changes with each backend switch. Code that caches stored data reads it
 * before and after a wait, and discards its result when the value changed.
 */
export const storageGeneration = () => storageRevision;
/**
 * True for a `DOMException` that means browser storage is full or unusable. The app answers
 * with a temporary session. A `ReplicaConflict` and other errors are not storage failures.
 */
export const isStorageFailure = (error: unknown) =>
    error instanceof DOMException &&
    ['QuotaExceededError', 'SecurityError', 'UnknownError', 'InvalidStateError', 'VersionError'].includes(error.name);
/**
 * Switches this tab to the memory backend and closes the database connection.
 * The memory backend starts empty: nothing is copied from the library.
 */
export function useTemporaryStorage() {
    if (!temporary) storageRevision++;
    temporary = true;
    const closing = dbPromise;
    dbPromise = null;
    closing?.then((database) => database.close()).catch(() => {});
}

// An IndexedDB request as a promise.
const req = <T>(r: IDBRequest<T>) =>
    new Promise<T>((resolve, reject) => {
        r.onsuccess = () => resolve(r.result);
        r.onerror = () => reject(r.error);
    });

// Opens the database once and returns the shared connection. The first open creates the three
// stores. A failed open stays cached, so later calls fail until `useTemporaryStorage()`.
function db() {
    dbPromise ??= new Promise((resolve, reject) => {
        // Open the existing version, including databases upgraded by the optional review feature.
        // A rollback must preserve documents and extra stores, not attempt a version downgrade.
        const open = indexedDB.open(DB_NAME);
        open.onupgradeneeded = () => {
            const d = open.result;
            d.createObjectStore('nodes', { keyPath: 'id' });
            d.createObjectStore('docs');
            d.createObjectStore('meta');
        };
        open.onsuccess = () => {
            // Another tab wants to upgrade or delete the database. Close this connection so that it is
            // not blocked. The next call opens the database again.
            open.result.onversionchange = () => {
                open.result.close();
                dbPromise = null;
            };
            resolve(open.result);
        };
        open.onerror = () => reject(open.error);
        open.onblocked = () => reject(new Error('Close other MARS tabs to finish upgrading storage.'));
    });
    return dbPromise;
}

// Runs `run` in one transaction and resolves with its result after the commit.
// `run` must wait only for requests of this transaction. A wait on anything else lets the
// transaction commit early. When `run` or a request fails, the transaction aborts, no write
// stays, and the error propagates.
async function tx<T>(
    stores: string[],
    mode: IDBTransactionMode,
    run: (t: IDBTransaction) => Promise<T> | T,
): Promise<T> {
    const t = (await db()).transaction(stores, mode);
    const done = new Promise<void>((resolve, reject) => {
        t.oncomplete = () => resolve();
        // A failed request bubbles here before the transaction aborts, while `t.error` is still null.
        t.onerror = (event) =>
            reject((event.target as IDBRequest | null)?.error ?? t.error ?? new Error('Storage request failed'));
        t.onabort = () => reject(t.error ?? new Error('Storage transaction aborted'));
    });
    try {
        const result = await run(t);
        await done;
        return result;
    } catch (error) {
        try {
            t.abort();
        } catch {}
        await done.catch(() => {});
        throw error;
    }
}

// Other tabs refresh their tree when this one writes. The channel carries node ids only,
// never names or text. A temporary session neither sends nor receives.
let channel: BroadcastChannel | null = null;
const remoteListeners = new Set<(ids: string[]) => void>();
function openChannel() {
    if (channel || temporary) return;
    try {
        if ('BroadcastChannel' in globalThis && BroadcastChannel) {
            channel = new BroadcastChannel('mars');
            channel.addEventListener('message', (e) => {
                if (!temporary) {
                    syncDebug.record('library.refresh', { changed: e.data?.ids?.length ?? 0 });
                    for (const fn of remoteListeners) fn(e.data?.ids ?? []);
                }
            });
        }
    } catch {}
}
openChannel();
/**
 * Registers a listener for library writes of other tabs. `ids` are the changed node ids.
 * The list is empty for a change that is not tied to nodes, such as a metadata write.
 */
export const onRemoteChange = (fn: (ids: string[]) => void) => {
    remoteListeners.add(fn);
};
const announce = (ids: string[]) => {
    if (!temporary) {
        openChannel();
        channel?.postMessage({ ids });
    }
};

/** Every stored node, in no particular order. */
export const allNodes = () =>
    temporary
        ? Promise.resolve(memory.all<TreeNode>('nodes'))
        : tx(['nodes'], 'readonly', (t) => req(t.objectStore('nodes').getAll() as IDBRequest<TreeNode[]>));
/**
 * Marks every stored node of one connection (collection and member) as `ended`, in one
 * transaction. `ended` is the only phase that survives a reload. It sends no broadcast.
 */
export async function markConnectionEnded(collection: string, member: string) {
    const mark = (node: TreeNode) =>
        node.connection?.collection === collection && node.connection.member === member
            ? { ...node, connection: { ...node.connection, phase: 'ended' as const } }
            : null;
    if (temporary) {
        for (const node of memory.all<TreeNode>('nodes')) {
            const ended = mark(node);
            if (ended) memory.put('nodes', node.id, ended);
        }
        return;
    }
    await tx(
        ['nodes'],
        'readwrite',
        (t) =>
            new Promise<void>((resolve, reject) => {
                const store = t.objectStore('nodes'),
                    nodes = store.getAll();
                nodes.onerror = () => reject(nodes.error);
                nodes.onsuccess = () => {
                    for (const node of nodes.result as TreeNode[]) {
                        const ended = mark(node);
                        if (ended) store.put(ended);
                    }
                    resolve();
                };
            }),
    );
}
/** The stored text of a document, or undefined. */
export const getText = (id: string) =>
    temporary
        ? Promise.resolve(memory.get<string>('docs', id))
        : tx(['docs'], 'readonly', (t) => req(t.objectStore('docs').get(id) as IDBRequest<string | undefined>));

/**
 * Inserts or replaces nodes and document texts (`texts`: id -> text) in one transaction, then
 * broadcasts the node ids. A node and its text therefore never disagree after a crash.
 */
export async function putNodes(nodes: TreeNode[], texts: Record<string, string> = {}) {
    if (temporary) {
        for (const node of nodes) memory.put('nodes', node.id, node);
        for (const [id, text] of Object.entries(texts)) memory.put('docs', id, text);
        return;
    }
    await tx(['nodes', 'docs'], 'readwrite', (t) => {
        for (const n of nodes) t.objectStore('nodes').put(n);
        for (const [id, text] of Object.entries(texts)) t.objectStore('docs').put(text, id);
    });
    announce(nodes.map((n) => n.id));
}

/**
 * Deletes nodes with their texts and every side record in one transaction, then broadcasts.
 * It does not look for descendants. The caller passes the complete subtree.
 */
export async function deleteNodes(ids: string[]) {
    if (temporary) {
        for (const id of ids) {
            memory.delete('nodes', id);
            memory.delete('docs', id);
            for (const kind of SIDES) memory.delete('docs', sideKey(kind, id));
        }
        return;
    }
    await tx(['nodes', 'docs'], 'readwrite', (t) => {
        for (const id of ids) {
            t.objectStore('nodes').delete(id);
            t.objectStore('docs').delete(id);
            for (const kind of SIDES) t.objectStore('docs').delete(sideKey(kind, id));
        }
    });
    announce(ids);
}

/**
 * Per-document side data in the `docs` store, so it needs no new store.
 * - `base`: the text at the last review, kept while changes are unreviewed.
 * - `opened`: the source text as MARS last received it.
 * - `vec`: the smart-search index entry.
 * - `sync`: the replica record of a connected node (state, outbox, keys, and progress).
 * - `sync-delete`: a durable deletion job, keyed by its own UUID rather than a document ID.
 */
export type Side = 'base' | 'opened' | 'vec' | 'sync' | 'sync-delete';
// Every kind. `deleteNodes` uses the list, so a new kind must be added here.
const SIDES: Side[] = ['base', 'opened', 'vec', 'sync', 'sync-delete'];
// The key of a side record in the `docs` store: `<kind>:<id>`.
const sideKey = (kind: Side, id: string) => `${kind}:${id}`;
/** One side record, or undefined. */
export const getSide = <T = string>(kind: Side, id: string) =>
    temporary
        ? Promise.resolve(memory.get<T>('docs', sideKey(kind, id)))
        : tx(['docs'], 'readonly', (t) =>
              req(t.objectStore('docs').get(sideKey(kind, id)) as IDBRequest<T | undefined>),
          );
/** Inserts or replaces one side record. It sends no broadcast. */
export const putSide = async (kind: Side, id: string, value: unknown) => {
    if (temporary) memory.put('docs', sideKey(kind, id), value);
    else
        await tx(['docs'], 'readwrite', (t) => {
            t.objectStore('docs').put(value, sideKey(kind, id));
        });
};
/** Deletes one side record. A missing record is not an error. */
export const deleteSide = async (kind: Side, id: string) => {
    if (temporary) memory.delete('docs', sideKey(kind, id));
    else
        await tx(['docs'], 'readwrite', (t) => {
            t.objectStore('docs').delete(sideKey(kind, id));
        });
};
// The key range that holds every record of one kind.
const sideRange = (kind: Side) => IDBKeyRange.bound(`${kind}:`, `${kind}:\uffff`);
/** The values of every side record of one kind, without their ids. */
export const allSides = <T>(kind: Side) =>
    temporary
        ? Promise.resolve(memory.all<T>('docs', `${kind}:`))
        : tx(['docs'], 'readonly', (t) => req(t.objectStore('docs').getAll(sideRange(kind)) as IDBRequest<T[]>));
/** Deletes every side record of one kind, for example to reset the search index. */
export const clearSides = async (kind: Side) => {
    if (temporary) memory.clearPrefix('docs', `${kind}:`);
    else
        await tx(['docs'], 'readwrite', (t) => {
            t.objectStore('docs').delete(sideRange(kind));
        });
};

/** One value of the `meta` store, or undefined. */
export const getMeta = <T>(key: string) =>
    temporary
        ? Promise.resolve(memory.get<T>('meta', key))
        : tx(['meta'], 'readonly', (t) => req(t.objectStore('meta').get(key) as IDBRequest<T | undefined>));
/**
 * Inserts or replaces one `meta` value. With `broadcast`, other tabs receive an empty change
 * list. A temporary session keeps the value in memory and never broadcasts.
 */
export const setMeta = async (key: string, value: unknown, broadcast = false) => {
    if (temporary) memory.put('meta', key, value);
    else
        await tx(['meta'], 'readwrite', (t) => {
            t.objectStore('meta').put(value, key);
        });
    if (broadcast) announce([]);
};

/**
 * Applies verified personal-Sync deletions in one transaction. Only this collection/member is affected.
 * Connected descendants of deleted folders also leave Sync. Independent descendants move to the top level.
 * Pending work keeps its text, review side data, and vector under Recovered Sync copies, with a complete
 * encrypted-recovery-compatible scope archive. An optional durable upload job commits with the deletion.
 * Storage or archive failure writes nothing. The caller verifies all remote receipts before calling.
 * `draft` synchronously captures current editor text after storage reads, before the first write.
 */
export async function deleteConnected(
    collection: string,
    member: string,
    documents: string[],
    job?: { id: string; value: unknown },
    draft?: () => { id: string; text: string } | undefined,
) {
    const scopeKey = `sync:scope:${collection}:${member}`;
    const run = async (
        all: TreeNode[],
        read: (store: 'docs' | 'meta', key: string) => Promise<any>,
        put: (store: 'nodes' | 'docs' | 'meta', key: string, value: unknown) => void,
        remove: (store: 'nodes' | 'docs', key: string) => void,
    ) => {
        const matches = (node: TreeNode) =>
            node.connection?.collection === collection && node.connection.member === member;
        const targets = new Set(documents);
        const gone = new Set(
            all.filter((node) => matches(node) && targets.has(node.connection!.document)).map((node) => node.id),
        );
        for (const node of all.filter((node) => gone.has(node.id) && node.kind === 'folder'))
            for (const child of descendants(all, node.id)) if (matches(child)) gone.add(child.id);
        const scoped = all.filter(matches);
        // A new connected child can appear while the caller signs the subtree. Retain the complete
        // selection and ask for a retry rather than silently deleting an item with no queued receipt.
        if (job && scoped.some((node) => gone.has(node.id) && !targets.has(node.connection!.document)))
            throw new ReplicaConflict();
        const scope = await read('meta', scopeKey);
        if (!scope) throw new ReplicaConflict('missing-node');
        const records = new Map<string, SessionRecord>();
        const texts: Record<string, string> = {};
        for (const node of scoped) {
            const [record, text] = await Promise.all([read('docs', sideKey('sync', node.id)), read('docs', node.id)]);
            if (record) records.set(node.id, record);
            if (typeof text === 'string') texts[node.id] = text;
        }
        let recovery = all.find(
            (node) =>
                node.kind === 'folder' &&
                !node.connection &&
                node.parentId === null &&
                node.name === 'Recovered Sync copies',
        );
        const metadata: Record<string, unknown> = { [scopeKey]: scope };
        let backups: UnfinishedSetupBackup[] = [];
        if (gone.size) {
            recovery ??= {
                id: crypto.randomUUID(),
                kind: 'folder',
                name: 'Recovered Sync copies',
                parentId: null,
                open: true,
                order: all.length,
                createdAt: Date.now(),
                updatedAt: Date.now(),
            };
            for (const key of [
                `sync:links:${scopeKey}`,
                `sync:rotation:${scopeKey}`,
                `sync:revoked-access:${scopeKey}`,
            ]) {
                const value = await read('meta', key);
                if (value !== undefined) metadata[key] = value;
            }
            backups = (await read('meta', unfinishedSetupIndex(recovery.id))) ?? [];
        }
        // Read the current editor only after the last asynchronous read. No event can add another
        // keystroke between this snapshot and the transaction's writes.
        const view = draft?.();
        const draftId = view && gone.has(view.id) && view.text !== texts[view.id] ? view.id : undefined;
        if (draftId) texts[draftId] = view!.text;
        const pending = scoped.filter(
            (node) =>
                gone.has(node.id) &&
                node.kind === 'doc' &&
                (records.get(node.id)?.outbox.length || node.id === draftId),
        );
        const writes: { store: 'nodes' | 'docs' | 'meta'; key: string; value: unknown }[] = [];
        if (pending.length) {
            const key = `sync:unfinished-setup:${crypto.randomUUID()}`,
                at = Date.now();
            const snapshot: SavedUnfinishedSetup = {
                v: 1,
                kind: 'mars-unfinished-sync',
                at,
                snapshot: { nodes: scoped, texts, records: Object.fromEntries(records), metadata },
            };
            const index = unfinishedSetupIndex(recovery!.id);
            writes.push(
                { store: 'meta', key, value: snapshot },
                { store: 'meta', key: index, value: [...backups, { key, at, records: records.size }] },
                { store: 'nodes', key: recovery!.id, value: recovery },
            );
        }
        for (const node of pending) {
            const record = records.get(node.id)!;
            const text = texts[node.id] ?? record.projection;
            const copy = {
                ...node,
                parentId: recovery!.id,
                originId: crypto.randomUUID(),
                manualFileSave: true,
                size: textBytes(text),
                threads: openThreads(text),
                updatedAt: node.id === draftId ? Date.now() : node.updatedAt,
            };
            delete copy.connection;
            writes.push({ store: 'nodes', key: node.id, value: copy }, { store: 'docs', key: node.id, value: text });
            // A remote commit can have advanced the projection while the visible text held a draft.
            if (record.outbox.length && record.projection !== texts[node.id]) {
                const id = crypto.randomUUID();
                writes.push(
                    {
                        store: 'nodes',
                        key: id,
                        value: {
                            ...copy,
                            id,
                            originId: id,
                            handle: undefined,
                            name: `${copy.name} (pending projection)`,
                            size: textBytes(record.projection),
                            threads: openThreads(record.projection),
                        },
                    },
                    { store: 'docs', key: id, value: record.projection },
                );
            }
        }
        for (const node of all)
            if (!gone.has(node.id) && node.parentId && gone.has(node.parentId))
                writes.push({ store: 'nodes', key: node.id, value: { ...node, parentId: null } });
        for (const node of scoped.filter((node) => gone.has(node.id))) targets.add(node.connection!.document);
        const exclusions = [...new Set([...scope.exclusions, ...targets])];
        writes.push({ store: 'meta', key: scopeKey, value: { ...scope, exclusions } });
        if (job) writes.push({ store: 'docs', key: sideKey('sync-delete', job.id), value: job.value });
        // Plan first, including cloneability, so the memory backend has the same all-or-nothing contract.
        structuredClone(writes);
        const kept = new Set(pending.map((node) => node.id));
        for (const id of gone) {
            remove('docs', sideKey('sync', id));
            if (kept.has(id)) continue;
            remove('nodes', id);
            remove('docs', id);
            for (const side of SIDES) remove('docs', sideKey(side, id));
        }
        for (const write of writes) put(write.store, write.key, write.value);
        return { removed: [...gone], recovered: pending.length, exclusions };
    };
    const result = temporary
        ? await run(
              memory.all('nodes'),
              async (store, key) => memory.get(store, key),
              (store, key, value) => memory.put(store, key, value),
              (store, key) => {
                  memory.delete(store, key);
              },
          )
        : await tx(['nodes', 'docs', 'meta'], 'readwrite', async (t) =>
              run(
                  await req(t.objectStore('nodes').getAll()),
                  (store, key) => req(t.objectStore(store).get(key)),
                  (store, key, value) => {
                      store === 'nodes' ? t.objectStore(store).put(value) : t.objectStore(store).put(value, key);
                  },
                  (store, key) => {
                      t.objectStore(store).delete(key);
                  },
              ),
          );
    announce(result.removed);
    return result;
}
/**
 * Reads a consistent snapshot of one connection (collection and member) for a recovery export.
 * One read-only transaction supplies every part, so the parts match each other.
 * - `nodes`: the connected nodes, limited to `ids` when given.
 * - `excludedDocuments`: remote document ids of the connected nodes that `ids` leaves out.
 * - `records`: the `sync` side records of `nodes`.
 * - `metadata`: the `meta` value of each key in `metadataKeys`.
 */
export async function captureConnected(collection: string, member: string, metadataKeys: string[], ids?: string[]) {
    const selection = ids ? new Set(ids) : null;
    const matches = (node: TreeNode) => node.connection?.collection === collection && node.connection.member === member;
    const assemble = (nodes: TreeNode[], records: unknown[], metadata: unknown[]) => ({
        nodes: nodes.filter((node) => matches(node) && (!selection || selection.has(node.id))),
        excludedDocuments: nodes
            .filter((node) => matches(node) && selection && !selection.has(node.id))
            .map((node) => node.connection!.document),
        records,
        metadata: Object.fromEntries(metadataKeys.map((key, index) => [key, metadata[index]])),
    });
    const selected = (nodes: TreeNode[]) =>
        nodes.filter((node) => matches(node) && (!selection || selection.has(node.id)));
    if (temporary) {
        const all = memory.all<TreeNode>('nodes'),
            nodes = selected(all);
        return assemble(
            all,
            nodes.map((node) => memory.get('docs', sideKey('sync', node.id))).filter((value) => value != null),
            metadataKeys.map((key) => memory.get('meta', key)),
        );
    }
    return tx(
        ['nodes', 'docs', 'meta'],
        'readonly',
        (t) =>
            new Promise<ReturnType<typeof assemble>>((resolve, reject) => {
                const metadata = Promise.all(metadataKeys.map((key) => req(t.objectStore('meta').get(key))));
                metadata.catch(reject);
                const read = t.objectStore('nodes').getAll();
                read.onerror = () => reject(read.error);
                read.onsuccess = () => {
                    const nodes = selected(read.result as TreeNode[]);
                    const records = Promise.all(
                        nodes.map((node) => req(t.objectStore('docs').get(sideKey('sync', node.id)))),
                    );
                    Promise.all([records, metadata]).then(
                        ([records, metadata]) =>
                            resolve(
                                assemble(
                                    read.result as TreeNode[],
                                    records.filter((value) => value != null),
                                    metadata,
                                ),
                            ),
                        reject,
                    );
                };
            }),
    );
}

/**
 * Thrown when stored state differs from the state that the caller prepared against.
 * The call wrote nothing. `reason` is a fixed diagnostic code,
 * and `field` names the node field for `metadata-changed`. Each construction records a
 * `store.conflict` trace event. The usual answer is to read the current state and retry.
 */
export class ReplicaConflict extends Error {
    readonly reason: ConflictReason;
    readonly field?: string;
    constructor(reason: ConflictReason = 'revision', field?: string) {
        super('The connected replica changed in another tab. Retry with its current state.');
        this.name = 'ReplicaConflict';
        this.reason = reason;
        this.field = field;
        syncDebug.record('store.conflict', { reason, field });
    }
}
/**
 * Publishes a downloaded folder hierarchy in one transaction and broadcasts it.
 * Network/decryption work has finished. Resolve remote identities AGAIN inside
 * the shared write transaction, then publish nodes, bodies and hierarchy together.
 *
 * @param root The local root node of the connection.
 * @param body The body of the root replica that the hierarchy was prepared against.
 * @param entries Incoming nodes, each parent before its children. `parent` is a remote
 *   document id. `record` is required for a document that is not stored yet.
 * @param bootstrap First publication of the connection: the root record and its metadata.
 * @returns Every node of the connection after the publication.
 *
 * A node that another tab already stored for the same remote document keeps its id and its
 * other fields. Only parent, name, and order change.
 * Throws `ReplicaConflict`:
 * - `already-connected`: two rows hold one remote document, or a bootstrap finds a stored root.
 * - `missing-node`: a parent is not a known folder.
 * - `revision`: the stored root or its body changed, or a new node cannot be added.
 */
export async function materializeConnected(
    root: TreeNode,
    body: string,
    entries: { node: TreeNode; parent: string; record?: SessionRecord }[],
    bootstrap?: { record: SessionRecord; metadata: Record<string, unknown> },
) {
    const binding = root.connection!;
    const publish = (
        all: TreeNode[],
        storedBody: string | undefined,
        write: (node: TreeNode, record?: SessionRecord) => void,
        meta: (key: string, value: unknown) => void,
    ) => {
        const connected = all.filter(
            (node) => node.connection?.collection === binding.collection && node.connection.member === binding.member,
        );
        const mapping = new Map<string, TreeNode>();
        for (const node of connected) {
            if (mapping.has(node.connection!.document)) throw new ReplicaConflict('already-connected');
            mapping.set(node.connection!.document, node);
        }
        const storedRoot = mapping.get(binding.document);
        if (bootstrap) {
            if (storedRoot || all.some((node) => node.id === root.id)) throw new ReplicaConflict('already-connected');
            mapping.set(binding.document, root);
        } else if (storedRoot?.id !== root.id || storedBody !== body) throw new ReplicaConflict();
        const updates: { node: TreeNode; record?: SessionRecord }[] = bootstrap
            ? [{ node: root, record: bootstrap.record }]
            : [];
        for (const entry of entries) {
            const incoming = entry.node,
                remote = incoming.connection!.document,
                previous = mapping.get(remote),
                parent = mapping.get(entry.parent);
            if (!parent || parent.kind !== 'folder') throw new ReplicaConflict('missing-node');
            if (
                !previous &&
                (all.some((node) => node.id === incoming.id) || (incoming.kind === 'doc' && !entry.record))
            )
                throw new ReplicaConflict();
            const node = previous
                ? { ...previous, parentId: parent.id, name: incoming.name, order: incoming.order }
                : { ...incoming, parentId: parent.id };
            mapping.set(remote, node);
            updates.push({ node, ...(!previous && entry.record ? { record: entry.record } : {}) });
        }
        // Validate the entire plan before the first write, including memory mode.
        if (temporary) structuredClone({ updates, metadata: bootstrap?.metadata });
        for (const update of updates) write(update.node, update.record);
        for (const [key, value] of Object.entries(bootstrap?.metadata ?? {})) meta(key, value);
        return [...mapping.values()];
    };
    let nodes: TreeNode[];
    if (temporary)
        nodes = publish(
            memory.all('nodes'),
            memory.get<SessionRecord>('docs', sideKey('sync', root.id))?.body,
            (node, record) => {
                memory.put('nodes', node.id, node);
                if (record) {
                    memory.put('docs', sideKey('sync', node.id), record);
                    if (node.kind === 'doc') memory.put('docs', node.id, record.projection);
                }
            },
            (key, value) => memory.put('meta', key, value),
        );
    else
        nodes = await tx(['nodes', 'docs', 'meta'], 'readwrite', async (t) => {
            const store = t.objectStore('nodes'),
                docs = t.objectStore('docs');
            const [all, record, scope] = await Promise.all([
                req(store.getAll()),
                req(docs.get(sideKey('sync', root.id))),
                req(t.objectStore('meta').get(`sync:scope:${binding.collection}:${binding.member}`)),
            ]);
            if (entries.some((entry) => scope?.exclusions?.includes(entry.node.connection!.document)))
                throw new ReplicaConflict();
            return publish(
                all,
                record?.body,
                (node, record) => {
                    store.put(node);
                    if (record) {
                        docs.put(record, sideKey('sync', node.id));
                        if (node.kind === 'doc') docs.put(record.projection, node.id);
                    }
                },
                (key, value) => {
                    t.objectStore('meta').put(value, key);
                },
            );
        });
    announce(nodes.map((node) => node.id));
    return nodes;
}
/**
 * A copy of local Sync state: nodes, texts by node id, `sync` records by node id, and `meta`
 * values by key. It is taken before a repair or a new setup replaces that state.
 */
export interface UnfinishedSetupSnapshot {
    nodes: TreeNode[];
    texts: Record<string, string>;
    records: Record<string, unknown>;
    metadata: Record<string, unknown>;
}
/** A snapshot as archived in the `meta` store under `sync:unfinished-setup:<uuid>`. `at` is in milliseconds. */
export interface SavedUnfinishedSetup {
    v: 1;
    kind: 'mars-unfinished-sync';
    at: number;
    snapshot: UnfinishedSetupSnapshot;
}
/** One entry of the archive index: the archive key, its time in milliseconds, and its number of records. */
export interface UnfinishedSetupBackup {
    key: string;
    at: number;
    records: number;
}
/** The `meta` key of the archive index for the local root node `id`. */
export const unfinishedSetupIndex = (id: string) => `sync:unfinished-backups:${id}`;
/**
 * Removes duplicate rows of one remote identity (collection, member, and document).
 * Old clients could create several rows for one remote identity. Archive the
 * complete scoped state first. Keep distinct visible text as local-only copies;
 * neither names nor thread counts are evidence that two documents are equal.
 *
 * One transaction covers the archive and the repair, so a failed archive leaves every row
 * as it was. The row that is the scope root, or else the oldest row, keeps the connection.
 * Returns true and broadcasts when it changed the library. Returns false in a temporary
 * session and when no duplicate exists. Throws `ReplicaConflict('missing-node')` when the scope
 * metadata of a duplicate names no local root.
 */
export async function repairDuplicateConnections() {
    if (temporary) return false;
    const changed = await tx(['nodes', 'docs', 'meta'], 'readwrite', async (t) => {
        const nodes = t.objectStore('nodes'),
            docs = t.objectStore('docs'),
            meta = t.objectStore('meta');
        const all = await req(nodes.getAll() as IDBRequest<TreeNode[]>),
            grouped = new Map<string, TreeNode[]>();
        for (const node of all)
            if (node.connection) {
                const { collection, member, document } = node.connection,
                    key = `${collection}:${member}:${document}`;
                grouped.set(key, [...(grouped.get(key) ?? []), node]);
            }
        const duplicates = [...grouped.values()].filter((group) => group.length > 1);
        if (!duplicates.length) return false;
        // Step 1: archive the complete state of each affected scope, once for each scope.
        const snapshots = new Map<string, { root: string; snapshot: UnfinishedSetupSnapshot }>();
        for (const group of duplicates) {
            const { collection, member } = group[0].connection!,
                key = `sync:scope:${collection}:${member}`;
            if (snapshots.has(key)) continue;
            const scope = (await req(meta.get(key))) ?? (await req(meta.get(`sync:scope:${collection}`)));
            if (!scope?.localRoot) throw new ReplicaConflict('missing-node');
            const scoped = all.filter(
                (node) => node.connection?.collection === collection && node.connection.member === member,
            );
            const snapshot: UnfinishedSetupSnapshot = { nodes: scoped, texts: {}, records: {}, metadata: {} };
            for (const node of scoped) {
                const text = await req(docs.get(node.id)),
                    record = await req(docs.get(sideKey('sync', node.id)));
                if (typeof text === 'string') snapshot.texts[node.id] = text;
                if (record) snapshot.records[node.id] = record;
            }
            for (const name of [
                key,
                `sync:scope:${collection}`,
                `sync:links:${key}`,
                `sync:rotation:${key}`,
                `sync:revoked-access:${key}`,
            ]) {
                const value = await req(meta.get(name));
                if (value !== undefined) snapshot.metadata[name] = value;
            }
            snapshots.set(key, { root: scope.localRoot, snapshot });
        }
        for (const { root, snapshot } of snapshots.values()) {
            const key = `sync:unfinished-setup:${crypto.randomUUID()}`,
                at = Date.now(),
                index = unfinishedSetupIndex(root);
            const backups = (await req(meta.get(index))) ?? [];
            meta.add({ v: 1, kind: 'mars-unfinished-sync', at, snapshot } satisfies SavedUnfinishedSetup, key);
            meta.put([...backups, { key, at, records: Object.keys(snapshot.records).length }], index);
        }
        // Step 2: one row of each group keeps the connection. Each other document row becomes a local
        // copy when it holds anything distinct. Otherwise the row is deleted.
        const replacements = new Map<string, string>();
        let recovery: TreeNode | undefined;
        for (const group of duplicates) {
            const { collection, member } = group[0].connection!,
                saved = snapshots.get(`sync:scope:${collection}:${member}`)!;
            group.sort(
                (a, b) =>
                    Number(b.id === saved.root) - Number(a.id === saved.root) ||
                    a.createdAt - b.createdAt ||
                    a.id.localeCompare(b.id),
            );
            const [keep, ...extras] = group;
            for (const node of extras) {
                replacements.set(node.id, keep.id);
                const text = saved.snapshot.texts[node.id],
                    canonical = saved.snapshot.texts[keep.id];
                const record = saved.snapshot.records[node.id] as SessionRecord | undefined,
                    previous = saved.snapshot.records[keep.id] as SessionRecord | undefined;
                const history = await Promise.all(
                    ['base', 'opened'].map(async (side) => {
                        const [extra, original] = await Promise.all([
                            req(docs.get(`${side}:${node.id}`)),
                            req(docs.get(`${side}:${keep.id}`)),
                        ]);
                        return extra !== original;
                    }),
                );
                if (
                    node.kind === 'doc' &&
                    (text !== canonical ||
                        record?.projection !== previous?.projection ||
                        node.handle ||
                        node.dirty ||
                        node.pending !== keep.pending ||
                        node.opened !== keep.opened ||
                        history.some(Boolean))
                ) {
                    recovery ??= {
                        id: crypto.randomUUID(),
                        kind: 'folder',
                        parentId: null,
                        name: 'Recovered Sync copies',
                        open: false,
                        order: all.length,
                        createdAt: Date.now(),
                        updatedAt: Date.now(),
                    };
                    const copy = {
                        ...node,
                        parentId: recovery.id,
                        originId: node.id,
                        size: textBytes(text ?? ''),
                        threads: openThreads(text ?? ''),
                    };
                    delete copy.connection;
                    nodes.put(copy);
                    // A stored projection can contain pending work not in the local text.
                    if (record && record.projection !== text) {
                        const id = crypto.randomUUID(),
                            projection = {
                                ...copy,
                                id,
                                originId: id,
                                handle: undefined,
                                name: `${node.name} (pending Sync work)`,
                                size: textBytes(record.projection),
                                threads: openThreads(record.projection),
                            };
                        nodes.add(projection);
                        docs.put(record.projection, projection.id);
                    }
                } else {
                    nodes.delete(node.id);
                    docs.delete(node.id);
                    for (const side of SIDES) docs.delete(sideKey(side, node.id));
                }
                docs.delete(sideKey('sync', node.id));
            }
        }
        // Step 3: the children of a replaced row move to the row that stays.
        for (const node of all)
            if (!replacements.has(node.id) && replacements.has(node.parentId ?? ''))
                nodes.put({ ...node, parentId: replacements.get(node.parentId!)! });
        if (recovery) nodes.add(recovery);
        return true;
    });
    if (changed) announce([]);
    return changed;
}
/**
 * Reads the nodes `ids` with their texts, their `sync` records, and the scope metadata of
 * those records, in one read-only transaction.
 * Throws in a temporary session. Throws `ReplicaConflict('missing-node')` when an id is gone.
 */
export async function captureUnfinishedSetup(ids: string[]): Promise<UnfinishedSetupSnapshot> {
    if (temporary) throw new Error('Recovery requires persistent library storage');
    return tx(['nodes', 'docs', 'meta'], 'readonly', async (t) => {
        const all = await req(t.objectStore('nodes').getAll() as IDBRequest<TreeNode[]>);
        const nodes = ids.map((id) => all.find((node) => node.id === id));
        if (nodes.some((node) => !node)) throw new ReplicaConflict('missing-node');
        const snapshot: UnfinishedSetupSnapshot = { nodes: nodes as TreeNode[], texts: {}, records: {}, metadata: {} },
            keys = new Set<string>();
        await Promise.all(
            snapshot.nodes.map(async (node) => {
                const docs = t.objectStore('docs');
                const [text, record] = await Promise.all([
                    req(docs.get(node.id)),
                    req(docs.get(sideKey('sync', node.id))),
                ]);
                if (typeof text === 'string') snapshot.texts[node.id] = text;
                else if (node.kind === 'doc') snapshot.texts[node.id] = '';
                if (record !== undefined) {
                    snapshot.records[node.id] = record;
                    if (
                        typeof record?.binding?.collection === 'string' &&
                        typeof record?.binding?.member === 'string'
                    ) {
                        const scope = `sync:scope:${record.binding.collection}:${record.binding.member}`;
                        for (const key of [
                            scope,
                            `sync:scope:${record.binding.collection}`,
                            `sync:links:${scope}`,
                            `sync:rotation:${scope}`,
                            `sync:revoked-access:${scope}`,
                        ])
                            keys.add(key);
                    }
                }
            }),
        );
        await Promise.all(
            [...keys].map(async (key) => {
                const value = await req(t.objectStore('meta').get(key));
                if (value !== undefined) snapshot.metadata[key] = value;
            }),
        );
        return snapshot;
    });
}
/**
 * Publish enrollment once, after all network registration and replica preparation.
 * Other tabs must never discover a half-enrolled folder or seed a competing replica.
 *
 * @param expected The selection as the caller read it before registration. The root is first.
 * @param texts The document texts as the caller read them, by node id.
 * @param nodes The same nodes, in the same order, with their `connection`.
 * @param records The new `sync` record of each node, by node id.
 * @param metadata `meta` values to write, by key.
 * @param previous An unfinished setup that this enrollment replaces. It is archived in the
 *   same transaction.
 *
 * Stored node fields other than `connection` and `manualFileSave` stay as they are.
 * Throws in a temporary session. Throws `ReplicaConflict` when the library changed since the read.
 * The reasons are `selection-grown`, `missing-node`, `already-connected`, `orphan-replica`,
 * `text-changed`, `metadata-changed` (with the field), and `revision` for changed scope metadata.
 */
export async function enrollConnected(
    expected: TreeNode[],
    texts: Record<string, string>,
    nodes: TreeNode[],
    records: Record<string, { revision: number }>,
    metadata: Record<string, unknown>,
    previous?: UnfinishedSetupSnapshot,
) {
    if (temporary) throw new Error('Sync requires persistent library storage');
    await tx(['nodes', 'docs', 'meta'], 'readwrite', async (t) => {
        const nodeStore = t.objectStore('nodes'),
            docs = t.objectStore('docs');
        const all = await req(nodeStore.getAll() as IDBRequest<TreeNode[]>),
            selected = new Set(expected.map((node) => node.id));
        // The folder gained a descendant after the caller read the selection.
        if (expected[0]?.kind === 'folder' && descendants(all, expected[0].id).some((node) => !selected.has(node.id)))
            throw new ReplicaConflict('selection-grown');
        const current = await Promise.all(
            expected.map(async (node) => {
                const [stored, text, replica] = await Promise.all([
                    req(nodeStore.get(node.id)),
                    req(docs.get(node.id)),
                    req(docs.get(sideKey('sync', node.id))),
                ]);
                if (!stored) throw new ReplicaConflict('missing-node');
                if (stored.connection) throw new ReplicaConflict('already-connected');
                // A stored replica must be the record of the unfinished setup that the caller saw, or absent.
                if (JSON.stringify(replica) !== JSON.stringify(previous?.records[node.id]))
                    throw new ReplicaConflict('orphan-replica');
                if (text !== texts[node.id]) throw new ReplicaConflict('text-changed');
                const field = ['name', 'parentId', 'order', 'originId', 'updatedAt', 'readingWidth'].find(
                    (key) => stored[key] !== node[key as keyof TreeNode],
                );
                if (field) throw new ReplicaConflict('metadata-changed', field);
                return stored as TreeNode;
            }),
        );
        if (previous) {
            for (const [key, value] of Object.entries(previous.metadata))
                if (JSON.stringify(await req(t.objectStore('meta').get(key))) !== JSON.stringify(value))
                    throw new ReplicaConflict();
            const root = expected[0].id,
                key = `sync:unfinished-setup:${crypto.randomUUID()}`,
                at = Date.now();
            const backups = (await req(t.objectStore('meta').get(unfinishedSetupIndex(root)))) as
                UnfinishedSetupBackup[] | undefined;
            t.objectStore('meta').add(
                { v: 1, kind: 'mars-unfinished-sync', at, snapshot: previous } satisfies SavedUnfinishedSetup,
                key,
            );
            t.objectStore('meta').put(
                [...(backups ?? []), { key, at, records: Object.keys(previous.records).length }],
                unfinishedSetupIndex(root),
            );
        }
        for (const [index, node] of nodes.entries())
            nodeStore.put({
                ...current[index],
                connection: node.connection,
                ...(node.manualFileSave ? { manualFileSave: true } : {}),
            });
        for (const [id, record] of Object.entries(records)) docs.put(record, sideKey('sync', id));
        for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
    });
    announce(nodes.map((node) => node.id));
}
/**
 * Compare-and-set of one `meta` record by its `revision`. `expected` null requires that no
 * record exists. `extras` are further `meta` values that commit in the same transaction.
 * Throws `ReplicaConflict` and writes nothing on a mismatch. Broadcasts after a library write.
 */
export async function commitMeta<T extends { revision: number }>(
    key: string,
    expected: number | null,
    value: T | null,
    extras: Record<string, unknown> = {},
) {
    if (temporary) {
        if ((memory.get<T>('meta', key)?.revision ?? null) !== expected) throw new ReplicaConflict();
        memory.put('meta', key, value);
        for (const [name, item] of Object.entries(extras)) memory.put('meta', name, item);
        return;
    }
    await tx(
        ['meta'],
        'readwrite',
        (t) =>
            new Promise<void>((resolve, reject) => {
                const meta = t.objectStore('meta'),
                    previous = meta.get(key);
                previous.onerror = () => reject(previous.error);
                previous.onsuccess = () => {
                    try {
                        if ((previous.result?.revision ?? null) !== expected) throw new ReplicaConflict();
                        meta.put(value, key);
                        for (const [name, item] of Object.entries(extras)) meta.put(item, name);
                        resolve();
                    } catch (error) {
                        reject(error);
                    }
                };
            }),
    );
    announce([]);
}
/**
 * Saves one replica record with everything that depends on it.
 * Prepared bytes only. Encryption and network work happen BEFORE this transaction.
 * Projection, CRDT state, outbox, bindings, and received progress commit together.
 * A stale same-profile tab cannot overwrite a newer replica or advance its cursor.
 *
 * @param id The local node id. The record is the `sync` side record of this node.
 * @param expectedRevision The stored revision that `replica` builds on. Null requires that no
 *   record exists.
 * @param nodes Node updates.
 * @param texts Markdown projections, by node id.
 * @param metadata `meta` values, by key.
 * @param extras Further `docs` store values, by complete key.
 * @param deletes `docs` store keys to delete.
 * @param binding With a non-null `expectedRevision`, the stored record and each stored node
 *   must carry this binding. Each node update then keeps the stored parent, order, folder
 *   state, and local reading width.
 *
 * Throws `ReplicaConflict` and writes nothing on a mismatch. Broadcasts the ids of `nodes`.
 */
export async function commitReplica<T extends { revision: number }>(
    id: string,
    expectedRevision: number | null,
    replica: T,
    nodes: TreeNode[] = [],
    texts: Record<string, string> = {},
    metadata: Record<string, unknown> = {},
    extras: Record<string, unknown> = {},
    deletes: string[] = [],
    binding?: { collection: string; document: string; member: string },
) {
    const key = sideKey('sync', id);
    const matches = (previous?: T & { binding?: typeof binding }) =>
        (previous?.revision ?? null) === expectedRevision &&
        (!binding ||
            expectedRevision === null ||
            (previous?.binding?.collection === binding.collection &&
                previous.binding.document === binding.document &&
                previous.binding.member === binding.member));
    const updatedNode = (node: TreeNode, stored?: TreeNode) => {
        if (!binding || expectedRevision === null) return node;
        if (
            stored?.connection?.collection !== binding.collection ||
            stored.connection.document !== binding.document ||
            stored.connection.member !== binding.member
        )
            throw new ReplicaConflict();
        // A body/review commit must not undo an atomically published tree move
        // or a local display preference captured before its asynchronous work.
        return {
            ...stored,
            ...node,
            parentId: stored.parentId,
            order: stored.order,
            open: stored.open,
            localReadingWidth: stored.localReadingWidth,
        };
    };
    if (temporary) {
        const previous = memory.get<T>('docs', key);
        if (!matches(previous)) throw new ReplicaConflict();
        const updates = nodes.map((node) => updatedNode(node, memory.get('nodes', node.id)));
        for (const node of updates) memory.put('nodes', node.id, node);
        for (const [doc, text] of Object.entries(texts)) memory.put('docs', doc, text);
        for (const [key, value] of Object.entries(extras)) memory.put('docs', key, value);
        for (const key of deletes) memory.delete('docs', key);
        for (const [name, value] of Object.entries(metadata)) memory.put('meta', name, value);
        memory.put('docs', key, replica);
        return;
    }
    await tx(['nodes', 'docs', 'meta'], 'readwrite', async (t) => {
        const docs = t.objectStore('docs'),
            existing = await req(docs.get(key));
        if (!matches(existing)) throw new ReplicaConflict();
        const updates = await Promise.all(
            nodes.map(async (node) => updatedNode(node, await req(t.objectStore('nodes').get(node.id)))),
        );
        for (const node of updates) t.objectStore('nodes').put(node);
        for (const [doc, text] of Object.entries(texts)) docs.put(text, doc);
        for (const [key, value] of Object.entries(extras)) docs.put(value, key);
        for (const key of deletes) docs.delete(key);
        for (const [name, value] of Object.entries(metadata)) t.objectStore('meta').put(value, name);
        docs.put(replica, key);
    });
    announce(nodes.map((n) => n.id));
}

/** Asks the browser not to evict reviews under storage pressure (granted silently in most browsers). */
export const persistStorage = () =>
    temporary ? Promise.resolve(false) : navigator.storage?.persist?.().catch(() => false);

/**
 * The only in-place exit from a temporary connected session. The caller supplies only
 * the explicitly selected document, not every temporary document or global preference.
 *
 * One transaction writes the node, its text, its `sync` record, its search vector, and
 * `metadata` to the library. After the commit this tab uses the IndexedDB backend.
 * Throws outside a temporary session. Throws `ReplicaConflict` when the library holds the
 * node id already. After a failure the tab stays in its temporary session.
 */
export async function promoteConnected(
    node: TreeNode,
    text: string,
    record: { revision: number },
    metadata: Record<string, unknown>,
) {
    if (!temporary) throw new Error('The session is already persistent');
    const vector = memory.get('docs', sideKey('vec', node.id));
    await tx(
        ['nodes', 'docs', 'meta'],
        'readwrite',
        (t) =>
            new Promise<void>((resolve, reject) => {
                const nodes = t.objectStore('nodes'),
                    existing = nodes.get(node.id);
                existing.onerror = () => reject(existing.error);
                existing.onsuccess = () => {
                    try {
                        if (existing.result) throw new ReplicaConflict();
                        nodes.put(node);
                        t.objectStore('docs').put(text, node.id);
                        t.objectStore('docs').put(record, sideKey('sync', node.id));
                        if (vector !== undefined) t.objectStore('docs').put(vector, sideKey('vec', node.id));
                        for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
                        resolve();
                    } catch (error) {
                        reject(error);
                    }
                };
            }),
    );
    temporary = false;
    storageRevision++;
    announce([node.id]);
}

/**
 * Recovery is all-or-nothing. Validation and decryption finish before this transaction.
 * Existing nodes and existing credentials cannot be overwritten by a partial restore.
 *
 * `records` holds the `sync` records by node id. The projection of a record becomes the text
 * of its document. `texts` holds the texts of restored documents that have no record.
 * Throws in a temporary session. Throws `ReplicaConflict` when a node id exists, and when the
 * library holds a node of the same connection.
 */
export async function restoreConnected(
    nodes: TreeNode[],
    records: Record<string, { revision: number; projection: string }>,
    metadata: Record<string, unknown>,
    texts: Record<string, string> = {},
) {
    if (temporary) throw new Error('Recovery requires persistent library storage');
    await tx(
        ['nodes', 'docs', 'meta'],
        'readwrite',
        (t) =>
            new Promise<void>((resolve, reject) => {
                const stored = t.objectStore('nodes').getAll();
                stored.onerror = () => reject(stored.error);
                stored.onsuccess = () => {
                    try {
                        const existing = stored.result as TreeNode[];
                        if (
                            nodes.some((node) =>
                                existing.some(
                                    (old) =>
                                        old.id === node.id ||
                                        (node.connection &&
                                            old.connection?.collection === node.connection.collection &&
                                            old.connection.member === node.connection.member),
                                ),
                            )
                        )
                            throw new ReplicaConflict();
                        for (const node of nodes) t.objectStore('nodes').add(node);
                        for (const [id, record] of Object.entries(records)) {
                            t.objectStore('docs').add(record, sideKey('sync', id));
                            if (nodes.find((node) => node.id === id)?.kind === 'doc')
                                t.objectStore('docs').add(record.projection, id);
                        }
                        for (const [id, text] of Object.entries(texts)) t.objectStore('docs').add(text, id);
                        for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
                        resolve();
                    } catch (error) {
                        reject(error);
                    }
                };
            }),
    );
    announce(nodes.map((node) => node.id));
}

/**
 * A confirmed local match retains its ID. Both copies are checked again inside the
 * transaction, so an edit from another tab cannot disappear behind the match dialog.
 *
 * @param incoming The id of the downloaded node. Its rows are deleted.
 * @param expected The local node as the match dialog saw it.
 * @param node The node to store under the local id.
 * @param localText The local text as the match dialog saw it.
 * @param record The replica record. Its revision must equal the stored revision of `incoming`.
 * @param metadata `meta` values to write, by key.
 * @param backup A node that keeps `localText` as a separate local document.
 *
 * Throws in a temporary session. Throws `ReplicaConflict` when the local node, its text, or
 * the downloaded replica changed, or when the local node is connected.
 */
export async function adoptConnected(
    incoming: string,
    expected: TreeNode,
    node: TreeNode,
    localText: string,
    record: { revision: number; projection: string },
    metadata: Record<string, unknown>,
    backup?: TreeNode,
) {
    if (temporary) throw new Error('Matching requires persistent library storage');
    await tx(
        ['nodes', 'docs', 'meta'],
        'readwrite',
        (t) =>
            new Promise<void>((resolve, reject) => {
                const nodes = t.objectStore('nodes'),
                    docs = t.objectStore('docs');
                const local = nodes.get(node.id),
                    previousText = docs.get(node.id),
                    replica = docs.get(sideKey('sync', incoming));
                // The three reads run in parallel. Continue when the last one is done.
                const finish = () => {
                    if (
                        local.readyState !== 'done' ||
                        previousText.readyState !== 'done' ||
                        replica.readyState !== 'done'
                    )
                        return;
                    try {
                        if (
                            !local.result ||
                            local.result.connection ||
                            previousText.result !== localText ||
                            replica.result?.revision !== record.revision ||
                            Object.keys(expected)
                                .filter((key) => key !== 'handle')
                                .some(
                                    (key) =>
                                        JSON.stringify(local.result[key]) !==
                                        JSON.stringify(expected[key as keyof TreeNode]),
                                ) ||
                            !!local.result.handle !== !!expected.handle
                        )
                            throw new ReplicaConflict();
                        if (backup) {
                            nodes.add(backup);
                            docs.add(localText, backup.id);
                        }
                        nodes.put(node);
                        docs.put(record.projection, node.id);
                        docs.put(record, sideKey('sync', node.id));
                        if (node.pending && !local.result.pending) docs.put(localText, sideKey('base', node.id));
                        nodes.delete(incoming);
                        docs.delete(incoming);
                        docs.delete(sideKey('sync', incoming));
                        for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
                        resolve();
                    } catch (error) {
                        reject(error);
                    }
                };
                for (const read of [local, previousText, replica]) {
                    read.onsuccess = finish;
                    read.onerror = () => reject(read.error);
                }
            }),
    );
    announce([incoming, node.id, ...(backup ? [backup.id] : [])]);
}
