// Review storage in IndexedDB: `nodes` (the folder/document tree), `docs` (document text,
// kept apart so listing the tree never loads bodies), and `meta` (small app settings).
import { descendants, textBytes, type TreeNode } from '../tree/model.ts';
import { openThreads } from '../share/merge.ts';
import { MemoryStore } from './memory.ts';
import { syncDebug, type ConflictReason } from '../sync/debug.ts';
import type { SessionRecord } from '../sync/session.ts';

const DB_NAME = 'mars';
let dbPromise: Promise<IDBDatabase> | null = null;
const memory = new MemoryStore();
const address = new URL(location.href);
let temporary = address.searchParams.get('temporary') === '1' || new URLSearchParams(address.hash.slice(1)).get('t') === '1'
    || new URLSearchParams(address.hash.slice(1)).has('live');
export const isTemporaryStorage = () => temporary;
let storageRevision = 0;
export const storageGeneration = () => storageRevision;
export const isStorageFailure = (error: unknown) => error instanceof DOMException && ['QuotaExceededError', 'SecurityError', 'UnknownError', 'InvalidStateError', 'VersionError'].includes(error.name);
export function useTemporaryStorage() {
    if (!temporary) storageRevision++;
    temporary = true;
    const closing = dbPromise; dbPromise = null;
    closing?.then((database) => database.close()).catch(() => {});
}

const req = <T>(r: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
});

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
            open.result.onversionchange = () => { open.result.close(); dbPromise = null; };
            resolve(open.result);
        };
        open.onerror = () => reject(open.error);
        open.onblocked = () => reject(new Error('Close other MARS tabs to finish upgrading storage.'));
    });
    return dbPromise;
}

async function tx<T>(stores: string[], mode: IDBTransactionMode, run: (t: IDBTransaction) => Promise<T> | T): Promise<T> {
    const t = (await db()).transaction(stores, mode);
    const done = new Promise<void>((resolve, reject) => {
        t.oncomplete = () => resolve();
        // A failed request bubbles here before the transaction aborts, while `t.error` is still null.
        t.onerror = (event) => reject((event.target as IDBRequest | null)?.error ?? t.error ?? new Error('Storage request failed'));
        t.onabort = () => reject(t.error ?? new Error('Storage transaction aborted'));
    });
    try {
        const result = await run(t);
        await done;
        return result;
    } catch (error) {
        try { t.abort(); } catch {}
        await done.catch(() => {});
        throw error;
    }
}

// Other tabs refresh their tree when this one writes.
let channel: BroadcastChannel | null = null;
const remoteListeners = new Set<(ids: string[]) => void>();
function openChannel() {
    if (channel || temporary) return;
    try {
        if ('BroadcastChannel' in globalThis && BroadcastChannel) {
            channel = new BroadcastChannel('mars');
            channel.addEventListener('message', (e) => { if (!temporary) { syncDebug.record('library.refresh', { changed: e.data?.ids?.length ?? 0 }); for (const fn of remoteListeners) fn(e.data?.ids ?? []); } });
        }
    } catch {}
}
openChannel();
export const onRemoteChange = (fn: (ids: string[]) => void) => { remoteListeners.add(fn); };
const announce = (ids: string[]) => { if (!temporary) { openChannel(); channel?.postMessage({ ids }); } };

export const allNodes = () => temporary ? Promise.resolve(memory.all<TreeNode>('nodes')) : tx(['nodes'], 'readonly', (t) => req(t.objectStore('nodes').getAll() as IDBRequest<TreeNode[]>));
export async function markConnectionEnded(collection: string, member: string) {
    const mark = (node: TreeNode) => node.connection?.collection === collection && node.connection.member === member ? { ...node, connection: { ...node.connection, phase: 'ended' as const } } : null;
    if (temporary) { for (const node of memory.all<TreeNode>('nodes')) { const ended = mark(node); if (ended) memory.put('nodes', node.id, ended); } return; }
    await tx(['nodes'], 'readwrite', t => new Promise<void>((resolve, reject) => {
        const store = t.objectStore('nodes'), nodes = store.getAll(); nodes.onerror = () => reject(nodes.error);
        nodes.onsuccess = () => { for (const node of nodes.result as TreeNode[]) { const ended = mark(node); if (ended) store.put(ended); } resolve(); };
    }));
}
export const getText = (id: string) => temporary ? Promise.resolve(memory.get<string>('docs', id)) : tx(['docs'], 'readonly', (t) => req(t.objectStore('docs').get(id) as IDBRequest<string | undefined>));

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

export async function deleteNodes(ids: string[]) {
    if (temporary) {
        for (const id of ids) {
            memory.delete('nodes', id); memory.delete('docs', id);
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

// Per-document side data in the `docs` store, so it needs no new store:
// `base` (the text at the last review, kept while changes are unreviewed), `opened` (the
// source text as MARS last received it), and `vec` (the smart-search index entry).
export type Side = 'base' | 'opened' | 'vec' | 'sync';
const SIDES: Side[] = ['base', 'opened', 'vec', 'sync'];
const sideKey = (kind: Side, id: string) => `${kind}:${id}`;
export const getSide = <T = string>(kind: Side, id: string) => temporary ? Promise.resolve(memory.get<T>('docs', sideKey(kind, id))) : tx(['docs'], 'readonly', (t) => req(t.objectStore('docs').get(sideKey(kind, id)) as IDBRequest<T | undefined>));
export const putSide = async (kind: Side, id: string, value: unknown) => {
    if (temporary) memory.put('docs', sideKey(kind, id), value);
    else await tx(['docs'], 'readwrite', (t) => { t.objectStore('docs').put(value, sideKey(kind, id)); });
};
export const deleteSide = async (kind: Side, id: string) => {
    if (temporary) memory.delete('docs', sideKey(kind, id));
    else await tx(['docs'], 'readwrite', (t) => { t.objectStore('docs').delete(sideKey(kind, id)); });
};
const sideRange = (kind: Side) => IDBKeyRange.bound(`${kind}:`, `${kind}:\uffff`);
export const allSides = <T>(kind: Side) => temporary ? Promise.resolve(memory.all<T>('docs', `${kind}:`)) : tx(['docs'], 'readonly', (t) => req(t.objectStore('docs').getAll(sideRange(kind)) as IDBRequest<T[]>));
export const clearSides = async (kind: Side) => {
    if (temporary) memory.clearPrefix('docs', `${kind}:`);
    else await tx(['docs'], 'readwrite', (t) => { t.objectStore('docs').delete(sideRange(kind)); });
};

export const getMeta = <T>(key: string) => temporary ? Promise.resolve(memory.get<T>('meta', key)) : tx(['meta'], 'readonly', (t) => req(t.objectStore('meta').get(key) as IDBRequest<T | undefined>));
export const setMeta = async (key: string, value: unknown, broadcast = false) => {
    if (temporary) memory.put('meta', key, value);
    else await tx(['meta'], 'readwrite', (t) => { t.objectStore('meta').put(value, key); });
    if (broadcast) announce([]);
};
export async function captureConnected(collection: string, member: string, metadataKeys: string[], ids?: string[]) {
    const selection = ids ? new Set(ids) : null;
    const matches = (node: TreeNode) => node.connection?.collection === collection && node.connection.member === member;
    const assemble = (nodes: TreeNode[], records: unknown[], metadata: unknown[]) => ({
        nodes: nodes.filter(node => matches(node) && (!selection || selection.has(node.id))),
        excludedDocuments: nodes.filter(node => matches(node) && selection && !selection.has(node.id)).map(node => node.connection!.document),
        records, metadata: Object.fromEntries(metadataKeys.map((key, index) => [key, metadata[index]])),
    });
    const selected = (nodes: TreeNode[]) => nodes.filter(node => matches(node) && (!selection || selection.has(node.id)));
    if (temporary) {
        const all = memory.all<TreeNode>('nodes'), nodes = selected(all);
        return assemble(all, nodes.map((node) => memory.get('docs', sideKey('sync', node.id))).filter((value) => value != null), metadataKeys.map((key) => memory.get('meta', key)));
    }
    return tx(['nodes', 'docs', 'meta'], 'readonly', (t) => new Promise<ReturnType<typeof assemble>>((resolve, reject) => {
        const metadata = Promise.all(metadataKeys.map((key) => req(t.objectStore('meta').get(key))));
        metadata.catch(reject);
        const read = t.objectStore('nodes').getAll(); read.onerror = () => reject(read.error);
        read.onsuccess = () => {
            const nodes = selected(read.result as TreeNode[]);
            const records = Promise.all(nodes.map((node) => req(t.objectStore('docs').get(sideKey('sync', node.id)))));
            Promise.all([records, metadata]).then(([records, metadata]) => resolve(assemble(read.result as TreeNode[], records.filter((value) => value != null), metadata)), reject);
        };
    }));
}

export class ReplicaConflict extends Error {
    readonly reason: ConflictReason;
    readonly field?: string;
    constructor(reason: ConflictReason = 'revision', field?: string) {
        super('The connected replica changed in another tab. Retry with its current state.'); this.name = 'ReplicaConflict';
        this.reason = reason; this.field = field;
        syncDebug.record('store.conflict', { reason, field });
    }
}
// Network/decryption work has finished. Resolve remote identities AGAIN inside
// the shared write transaction, then publish nodes, bodies and hierarchy together.
export async function materializeConnected(root: TreeNode, body: string, entries: { node: TreeNode; parent: string; record?: SessionRecord }[], bootstrap?: { record: SessionRecord; metadata: Record<string, unknown> }) {
    const binding = root.connection!;
    const publish = (all: TreeNode[], storedBody: string | undefined, write: (node: TreeNode, record?: SessionRecord) => void, meta: (key: string, value: unknown) => void) => {
        const connected = all.filter(node => node.connection?.collection === binding.collection && node.connection.member === binding.member);
        const mapping = new Map<string, TreeNode>();
        for (const node of connected) {
            if (mapping.has(node.connection!.document)) throw new ReplicaConflict('already-connected');
            mapping.set(node.connection!.document, node);
        }
        const storedRoot = mapping.get(binding.document);
        if (bootstrap) {
            if (storedRoot || all.some(node => node.id === root.id)) throw new ReplicaConflict('already-connected');
            mapping.set(binding.document, root);
        } else if (storedRoot?.id !== root.id || storedBody !== body) throw new ReplicaConflict();
        const updates: { node: TreeNode; record?: SessionRecord }[] = bootstrap ? [{ node: root, record: bootstrap.record }] : [];
        for (const entry of entries) {
            const incoming = entry.node, remote = incoming.connection!.document, previous = mapping.get(remote), parent = mapping.get(entry.parent);
            if (!parent || parent.kind !== 'folder') throw new ReplicaConflict('missing-node');
            if (!previous && (all.some(node => node.id === incoming.id) || incoming.kind === 'doc' && !entry.record)) throw new ReplicaConflict();
            const node = previous ? { ...previous, parentId: parent.id, name: incoming.name, order: incoming.order } : { ...incoming, parentId: parent.id };
            mapping.set(remote, node); updates.push({ node, ...(!previous && entry.record ? { record: entry.record } : {}) });
        }
        // Validate the entire plan before the first write, including memory mode.
        if (temporary) structuredClone({ updates, metadata: bootstrap?.metadata });
        for (const update of updates) write(update.node, update.record);
        for (const [key, value] of Object.entries(bootstrap?.metadata ?? {})) meta(key, value);
        return [...mapping.values()];
    };
    let nodes: TreeNode[];
    if (temporary) nodes = publish(memory.all('nodes'), memory.get<SessionRecord>('docs', sideKey('sync', root.id))?.body, (node, record) => {
        memory.put('nodes', node.id, node);
        if (record) { memory.put('docs', sideKey('sync', node.id), record); if (node.kind === 'doc') memory.put('docs', node.id, record.projection); }
    }, (key, value) => memory.put('meta', key, value));
    else nodes = await tx(['nodes', 'docs', 'meta'], 'readwrite', async t => {
        const store = t.objectStore('nodes'), docs = t.objectStore('docs');
        const [all, record] = await Promise.all([req(store.getAll()), req(docs.get(sideKey('sync', root.id)))]);
        return publish(all, record?.body, (node, record) => {
            store.put(node);
            if (record) { docs.put(record, sideKey('sync', node.id)); if (node.kind === 'doc') docs.put(record.projection, node.id); }
        }, (key, value) => { t.objectStore('meta').put(value, key); });
    });
    announce(nodes.map(node => node.id)); return nodes;
}
export interface UnfinishedSetupSnapshot {
    nodes: TreeNode[]; texts: Record<string, string>; records: Record<string, unknown>; metadata: Record<string, unknown>;
}
export interface SavedUnfinishedSetup { v: 1; kind: 'mars-unfinished-sync'; at: number; snapshot: UnfinishedSetupSnapshot }
export interface UnfinishedSetupBackup { key: string; at: number; records: number }
export const unfinishedSetupIndex = (id: string) => `sync:unfinished-backups:${id}`;
// Old clients could create several rows for one remote identity. Archive the
// complete scoped state first. Keep distinct visible text as local-only copies;
// neither names nor thread counts are evidence that two documents are equal.
export async function repairDuplicateConnections() {
    if (temporary) return false;
    const changed = await tx(['nodes', 'docs', 'meta'], 'readwrite', async t => {
        const nodes = t.objectStore('nodes'), docs = t.objectStore('docs'), meta = t.objectStore('meta');
        const all = await req(nodes.getAll() as IDBRequest<TreeNode[]>), grouped = new Map<string, TreeNode[]>();
        for (const node of all) if (node.connection) {
            const { collection, member, document } = node.connection, key = `${collection}:${member}:${document}`;
            grouped.set(key, [...(grouped.get(key) ?? []), node]);
        }
        const duplicates = [...grouped.values()].filter(group => group.length > 1);
        if (!duplicates.length) return false;
        const snapshots = new Map<string, { root: string; snapshot: UnfinishedSetupSnapshot }>();
        for (const group of duplicates) {
            const { collection, member } = group[0].connection!, key = `sync:scope:${collection}:${member}`;
            if (snapshots.has(key)) continue;
            const scope = await req(meta.get(key)) ?? await req(meta.get(`sync:scope:${collection}`));
            if (!scope?.localRoot) throw new ReplicaConflict('missing-node');
            const scoped = all.filter(node => node.connection?.collection === collection && node.connection.member === member);
            const snapshot: UnfinishedSetupSnapshot = { nodes: scoped, texts: {}, records: {}, metadata: {} };
            for (const node of scoped) {
                const text = await req(docs.get(node.id)), record = await req(docs.get(sideKey('sync', node.id)));
                if (typeof text === 'string') snapshot.texts[node.id] = text;
                if (record) snapshot.records[node.id] = record;
            }
            for (const name of [key, `sync:scope:${collection}`, `sync:links:${key}`, `sync:rotation:${key}`, `sync:revoked-access:${key}`]) {
                const value = await req(meta.get(name)); if (value !== undefined) snapshot.metadata[name] = value;
            }
            snapshots.set(key, { root: scope.localRoot, snapshot });
        }
        for (const { root, snapshot } of snapshots.values()) {
            const key = `sync:unfinished-setup:${crypto.randomUUID()}`, at = Date.now(), index = unfinishedSetupIndex(root);
            const backups = await req(meta.get(index)) ?? [];
            meta.add({ v: 1, kind: 'mars-unfinished-sync', at, snapshot } satisfies SavedUnfinishedSetup, key);
            meta.put([...backups, { key, at, records: Object.keys(snapshot.records).length }], index);
        }
        const replacements = new Map<string, string>();
        let recovery: TreeNode | undefined;
        for (const group of duplicates) {
            const { collection, member } = group[0].connection!, saved = snapshots.get(`sync:scope:${collection}:${member}`)!;
            group.sort((a, b) => Number(b.id === saved.root) - Number(a.id === saved.root) || a.createdAt - b.createdAt || a.id.localeCompare(b.id));
            const [keep, ...extras] = group;
            for (const node of extras) {
                replacements.set(node.id, keep.id);
                const text = saved.snapshot.texts[node.id], canonical = saved.snapshot.texts[keep.id];
                const record = saved.snapshot.records[node.id] as SessionRecord | undefined, previous = saved.snapshot.records[keep.id] as SessionRecord | undefined;
                const history = await Promise.all(['base', 'opened'].map(async side => {
                    const [extra, original] = await Promise.all([req(docs.get(`${side}:${node.id}`)), req(docs.get(`${side}:${keep.id}`))]);
                    return extra !== original;
                }));
                if (node.kind === 'doc' && (text !== canonical || record?.projection !== previous?.projection || node.handle || node.dirty || node.pending !== keep.pending || node.opened !== keep.opened || history.some(Boolean))) {
                    recovery ??= { id: crypto.randomUUID(), kind: 'folder', parentId: null, name: 'Recovered Sync copies', open: false, order: all.length, createdAt: Date.now(), updatedAt: Date.now() };
                    const copy = { ...node, parentId: recovery.id, originId: node.id, size: textBytes(text ?? ''), threads: openThreads(text ?? '') }; delete copy.connection; nodes.put(copy);
                    // A stored projection can contain pending work not in the local text.
                    if (record && record.projection !== text) {
                        const id = crypto.randomUUID(), projection = { ...copy, id, originId: id, handle: undefined, name: `${node.name} (pending Sync work)`, size: textBytes(record.projection), threads: openThreads(record.projection) };
                        nodes.add(projection); docs.put(record.projection, projection.id);
                    }
                } else { nodes.delete(node.id); docs.delete(node.id); for (const side of SIDES) docs.delete(sideKey(side, node.id)); }
                docs.delete(sideKey('sync', node.id));
            }
        }
        for (const node of all) if (!replacements.has(node.id) && replacements.has(node.parentId ?? '')) nodes.put({ ...node, parentId: replacements.get(node.parentId!)! });
        if (recovery) nodes.add(recovery);
        return true;
    });
    if (changed) announce([]);
    return changed;
}
export async function captureUnfinishedSetup(ids: string[]): Promise<UnfinishedSetupSnapshot> {
    if (temporary) throw new Error('Recovery requires persistent library storage');
    return tx(['nodes', 'docs', 'meta'], 'readonly', async t => {
        const all = await req(t.objectStore('nodes').getAll() as IDBRequest<TreeNode[]>);
        const nodes = ids.map(id => all.find(node => node.id === id));
        if (nodes.some(node => !node)) throw new ReplicaConflict('missing-node');
        const snapshot: UnfinishedSetupSnapshot = { nodes: nodes as TreeNode[], texts: {}, records: {}, metadata: {} }, keys = new Set<string>();
        await Promise.all(snapshot.nodes.map(async node => {
            const docs = t.objectStore('docs');
            const [text, record] = await Promise.all([req(docs.get(node.id)), req(docs.get(sideKey('sync', node.id)))]);
            if (typeof text === 'string') snapshot.texts[node.id] = text;
            else if (node.kind === 'doc') snapshot.texts[node.id] = '';
            if (record !== undefined) {
                snapshot.records[node.id] = record;
                if (typeof record?.binding?.collection === 'string' && typeof record?.binding?.member === 'string') {
                    const scope = `sync:scope:${record.binding.collection}:${record.binding.member}`;
                    for (const key of [scope, `sync:scope:${record.binding.collection}`, `sync:links:${scope}`, `sync:rotation:${scope}`, `sync:revoked-access:${scope}`]) keys.add(key);
                }
            }
        }));
        await Promise.all([...keys].map(async key => { const value = await req(t.objectStore('meta').get(key)); if (value !== undefined) snapshot.metadata[key] = value; }));
        return snapshot;
    });
}
// Publish enrollment once, after all network registration and replica preparation.
// Other tabs must never discover a half-enrolled folder or seed a competing replica.
export async function enrollConnected(expected: TreeNode[], texts: Record<string, string>, nodes: TreeNode[], records: Record<string, { revision: number }>, metadata: Record<string, unknown>, previous?: UnfinishedSetupSnapshot) {
    if (temporary) throw new Error('Sync requires persistent library storage');
    await tx(['nodes', 'docs', 'meta'], 'readwrite', async (t) => {
        const nodeStore = t.objectStore('nodes'), docs = t.objectStore('docs');
        const all = await req(nodeStore.getAll() as IDBRequest<TreeNode[]>), selected = new Set(expected.map(node => node.id));
        if (expected[0]?.kind === 'folder' && descendants(all, expected[0].id).some(node => !selected.has(node.id))) throw new ReplicaConflict('selection-grown');
        const current = await Promise.all(expected.map(async (node) => {
            const [stored, text, replica] = await Promise.all([req(nodeStore.get(node.id)), req(docs.get(node.id)), req(docs.get(sideKey('sync', node.id)))]);
            if (!stored) throw new ReplicaConflict('missing-node');
            if (stored.connection) throw new ReplicaConflict('already-connected');
            if (JSON.stringify(replica) !== JSON.stringify(previous?.records[node.id])) throw new ReplicaConflict('orphan-replica');
            if (text !== texts[node.id]) throw new ReplicaConflict('text-changed');
            const field = ['name', 'parentId', 'order', 'originId', 'updatedAt', 'readingWidth'].find(key => stored[key] !== node[key as keyof TreeNode]);
            if (field) throw new ReplicaConflict('metadata-changed', field);
            return stored as TreeNode;
        }));
        if (previous) {
            for (const [key, value] of Object.entries(previous.metadata)) if (JSON.stringify(await req(t.objectStore('meta').get(key))) !== JSON.stringify(value)) throw new ReplicaConflict();
            const root = expected[0].id, key = `sync:unfinished-setup:${crypto.randomUUID()}`, at = Date.now();
            const backups = await req(t.objectStore('meta').get(unfinishedSetupIndex(root))) as UnfinishedSetupBackup[] | undefined;
            t.objectStore('meta').add({ v: 1, kind: 'mars-unfinished-sync', at, snapshot: previous } satisfies SavedUnfinishedSetup, key);
            t.objectStore('meta').put([...(backups ?? []), { key, at, records: Object.keys(previous.records).length }], unfinishedSetupIndex(root));
        }
        for (const [index, node] of nodes.entries()) nodeStore.put({ ...current[index], connection: node.connection, ...(node.manualFileSave ? { manualFileSave: true } : {}) });
        for (const [id, record] of Object.entries(records)) docs.put(record, sideKey('sync', id));
        for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
    });
    announce(nodes.map(node => node.id));
}
export async function commitMeta<T extends { revision: number }>(key: string, expected: number | null, value: T | null, extras: Record<string, unknown> = {}) {
    if (temporary) {
        if ((memory.get<T>('meta', key)?.revision ?? null) !== expected) throw new ReplicaConflict();
        memory.put('meta', key, value); for (const [name, item] of Object.entries(extras)) memory.put('meta', name, item); return;
    }
    await tx(['meta'], 'readwrite', (t) => new Promise<void>((resolve, reject) => {
        const meta = t.objectStore('meta'), previous = meta.get(key);
        previous.onerror = () => reject(previous.error);
        previous.onsuccess = () => {
            try {
                if ((previous.result?.revision ?? null) !== expected) throw new ReplicaConflict();
                meta.put(value, key); for (const [name, item] of Object.entries(extras)) meta.put(item, name); resolve();
            } catch (error) { reject(error); }
        };
    }));
    announce([]);
}
// Prepared bytes only. Encryption and network work happen BEFORE this transaction.
// Projection, CRDT state, outbox, bindings, and received progress commit together.
// A stale same-profile tab cannot overwrite a newer replica or advance its cursor.
export async function commitReplica<T extends { revision: number }>(id: string, expectedRevision: number | null,
    replica: T, nodes: TreeNode[] = [], texts: Record<string, string> = {}, metadata: Record<string, unknown> = {}, extras: Record<string, unknown> = {}, deletes: string[] = [], binding?: { collection: string; document: string; member: string }) {
    const key = sideKey('sync', id);
    const matches = (previous?: T & { binding?: typeof binding }) => (previous?.revision ?? null) === expectedRevision
        && (!binding || expectedRevision === null || previous?.binding?.collection === binding.collection && previous.binding.document === binding.document && previous.binding.member === binding.member);
    const updatedNode = (node: TreeNode, stored?: TreeNode) => {
        if (!binding || expectedRevision === null) return node;
        if (stored?.connection?.collection !== binding.collection || stored.connection.document !== binding.document || stored.connection.member !== binding.member) throw new ReplicaConflict();
        // A body/review commit must not undo an atomically published tree move
        // or a local display preference captured before its asynchronous work.
        return { ...stored, ...node, parentId: stored.parentId, order: stored.order, open: stored.open, localReadingWidth: stored.localReadingWidth };
    };
    if (temporary) {
        const previous = memory.get<T>('docs', key);
        if (!matches(previous)) throw new ReplicaConflict();
        const updates = nodes.map(node => updatedNode(node, memory.get('nodes', node.id)));
        for (const node of updates) memory.put('nodes', node.id, node);
        for (const [doc, text] of Object.entries(texts)) memory.put('docs', doc, text);
        for (const [key, value] of Object.entries(extras)) memory.put('docs', key, value);
        for (const key of deletes) memory.delete('docs', key);
        for (const [name, value] of Object.entries(metadata)) memory.put('meta', name, value);
        memory.put('docs', key, replica);
        return;
    }
    await tx(['nodes', 'docs', 'meta'], 'readwrite', async t => {
        const docs = t.objectStore('docs'), existing = await req(docs.get(key));
        if (!matches(existing)) throw new ReplicaConflict();
        const updates = await Promise.all(nodes.map(async node => updatedNode(node, await req(t.objectStore('nodes').get(node.id)))));
        for (const node of updates) t.objectStore('nodes').put(node);
        for (const [doc, text] of Object.entries(texts)) docs.put(text, doc);
        for (const [key, value] of Object.entries(extras)) docs.put(value, key);
        for (const key of deletes) docs.delete(key);
        for (const [name, value] of Object.entries(metadata)) t.objectStore('meta').put(value, name);
        docs.put(replica, key);
    });
    announce(nodes.map((n) => n.id));
}

// Asks the browser not to evict reviews under storage pressure (granted silently in most browsers).
export const persistStorage = () => temporary ? Promise.resolve(false) : navigator.storage?.persist?.().catch(() => false);

// The only in-place exit from a temporary connected session. The caller supplies only
// the explicitly selected document, not every temporary document or global preference.
export async function promoteConnected(node: TreeNode, text: string, record: { revision: number }, metadata: Record<string, unknown>) {
    if (!temporary) throw new Error('The session is already persistent');
    const vector = memory.get('docs', sideKey('vec', node.id));
    await tx(['nodes', 'docs', 'meta'], 'readwrite', (t) => new Promise<void>((resolve, reject) => {
        const nodes = t.objectStore('nodes'), existing = nodes.get(node.id);
        existing.onerror = () => reject(existing.error);
        existing.onsuccess = () => {
            try {
                if (existing.result) throw new ReplicaConflict();
                nodes.put(node); t.objectStore('docs').put(text, node.id);
                t.objectStore('docs').put(record, sideKey('sync', node.id));
                if (vector !== undefined) t.objectStore('docs').put(vector, sideKey('vec', node.id));
                for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
                resolve();
            } catch (error) { reject(error); }
        };
    }));
    temporary = false;
    storageRevision++;
    announce([node.id]);
}

// Recovery is all-or-nothing. Validation and decryption finish before this transaction.
// Existing nodes and existing credentials cannot be overwritten by a partial restore.
export async function restoreConnected(nodes: TreeNode[], records: Record<string, { revision: number; projection: string }>, metadata: Record<string, unknown>, texts: Record<string, string> = {}) {
    if (temporary) throw new Error('Recovery requires persistent library storage');
    await tx(['nodes', 'docs', 'meta'], 'readwrite', (t) => new Promise<void>((resolve, reject) => {
        const stored = t.objectStore('nodes').getAll();
        stored.onerror = () => reject(stored.error);
        stored.onsuccess = () => {
            try {
                const existing = stored.result as TreeNode[];
                if (nodes.some((node) => existing.some((old) => old.id === node.id || node.connection && old.connection?.collection === node.connection.collection && old.connection.member === node.connection.member))) throw new ReplicaConflict();
                for (const node of nodes) t.objectStore('nodes').add(node);
                for (const [id, record] of Object.entries(records)) {
                    t.objectStore('docs').add(record, sideKey('sync', id));
                    if (nodes.find((node) => node.id === id)?.kind === 'doc') t.objectStore('docs').add(record.projection, id);
                }
                for (const [id, text] of Object.entries(texts)) t.objectStore('docs').add(text, id);
                for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
                resolve();
            } catch (error) { reject(error); }
        };
    }));
    announce(nodes.map((node) => node.id));
}

// A confirmed local match retains its ID. Both copies are checked again inside the
// transaction, so an edit from another tab cannot disappear behind the match dialog.
export async function adoptConnected(incoming: string, expected: TreeNode, node: TreeNode, localText: string,
    record: { revision: number; projection: string }, metadata: Record<string, unknown>, backup?: TreeNode) {
    if (temporary) throw new Error('Matching requires persistent library storage');
    await tx(['nodes', 'docs', 'meta'], 'readwrite', (t) => new Promise<void>((resolve, reject) => {
        const nodes = t.objectStore('nodes'), docs = t.objectStore('docs');
        const local = nodes.get(node.id), previousText = docs.get(node.id), replica = docs.get(sideKey('sync', incoming));
        const finish = () => {
            if (local.readyState !== 'done' || previousText.readyState !== 'done' || replica.readyState !== 'done') return;
            try {
                if (!local.result || local.result.connection || previousText.result !== localText || replica.result?.revision !== record.revision
                    || Object.keys(expected).filter((key) => key !== 'handle').some((key) => JSON.stringify(local.result[key]) !== JSON.stringify(expected[key as keyof TreeNode]))
                    || !!local.result.handle !== !!expected.handle) throw new ReplicaConflict();
                if (backup) { nodes.add(backup); docs.add(localText, backup.id); }
                nodes.put(node); docs.put(record.projection, node.id); docs.put(record, sideKey('sync', node.id));
                if (node.pending && !local.result.pending) docs.put(localText, sideKey('base', node.id));
                nodes.delete(incoming); docs.delete(incoming); docs.delete(sideKey('sync', incoming));
                for (const [key, value] of Object.entries(metadata)) t.objectStore('meta').put(value, key);
                resolve();
            } catch (error) { reject(error); }
        };
        for (const read of [local, previousText, replica]) { read.onsuccess = finish; read.onerror = () => reject(read.error); }
    }));
    announce([incoming, node.id, ...(backup ? [backup.id] : [])]);
}
