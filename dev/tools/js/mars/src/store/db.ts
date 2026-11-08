// Review storage in IndexedDB: `nodes` (the folder/document tree), `docs` (document text,
// kept apart so listing the tree never loads bodies), and `meta` (small app settings).
import type { TreeNode } from '../tree/model.ts';
import { MemoryStore } from './memory.ts';

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
        t.onerror = () => reject(t.error);
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
            channel.addEventListener('message', (e) => { if (!temporary) for (const fn of remoteListeners) fn(e.data?.ids ?? []); });
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
export const setMeta = async (key: string, value: unknown) => {
    if (temporary) memory.put('meta', key, value);
    else await tx(['meta'], 'readwrite', (t) => { t.objectStore('meta').put(value, key); });
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
    constructor() { super('The connected replica changed in another tab. Retry with its current state.'); this.name = 'ReplicaConflict'; }
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
    replica: T, nodes: TreeNode[] = [], texts: Record<string, string> = {}, metadata: Record<string, unknown> = {}, extras: Record<string, unknown> = {}, deletes: string[] = []) {
    const key = sideKey('sync', id);
    if (temporary) {
        const previous = memory.get<T>('docs', key);
        if ((previous?.revision ?? null) !== expectedRevision) throw new ReplicaConflict();
        for (const node of nodes) memory.put('nodes', node.id, node);
        for (const [doc, text] of Object.entries(texts)) memory.put('docs', doc, text);
        for (const [key, value] of Object.entries(extras)) memory.put('docs', key, value);
        for (const key of deletes) memory.delete('docs', key);
        for (const [name, value] of Object.entries(metadata)) memory.put('meta', name, value);
        memory.put('docs', key, replica);
        return;
    }
    await tx(['nodes', 'docs', 'meta'], 'readwrite', (t) => new Promise<void>((resolve, reject) => {
        const docs = t.objectStore('docs'), existing = docs.get(key);
        existing.onerror = () => reject(existing.error);
        existing.onsuccess = () => {
            try {
                if (((existing.result as T | undefined)?.revision ?? null) !== expectedRevision) throw new ReplicaConflict();
                for (const node of nodes) t.objectStore('nodes').put(node);
                for (const [doc, text] of Object.entries(texts)) docs.put(text, doc);
                for (const [key, value] of Object.entries(extras)) docs.put(value, key);
                for (const key of deletes) docs.delete(key);
                for (const [name, value] of Object.entries(metadata)) t.objectStore('meta').put(value, name);
                docs.put(replica, key);
                resolve();
            } catch (error) { reject(error); }
        };
    }));
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
