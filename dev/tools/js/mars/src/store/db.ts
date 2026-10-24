// Review storage in IndexedDB: `nodes` (the folder/document tree), `docs` (document text,
// kept apart so listing the tree never loads bodies), and `meta` (small app settings).
import type { TreeNode } from '../tree/model.ts';

const DB_NAME = 'mars';
let dbPromise: Promise<IDBDatabase> | null = null;

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
    const result = await run(t);
    await done;
    return result;
}

// Other tabs refresh their tree when this one writes.
const channel = 'BroadcastChannel' in globalThis ? new BroadcastChannel('mars') : null;
export const onRemoteChange = (fn: (ids: string[]) => void) => channel?.addEventListener('message', (e) => fn(e.data?.ids ?? []));
const announce = (ids: string[]) => channel?.postMessage({ ids });

export const allNodes = () => tx(['nodes'], 'readonly', (t) => req(t.objectStore('nodes').getAll() as IDBRequest<TreeNode[]>));
export const getText = (id: string) => tx(['docs'], 'readonly', (t) => req(t.objectStore('docs').get(id) as IDBRequest<string | undefined>));

export async function putNodes(nodes: TreeNode[], texts: Record<string, string> = {}) {
    await tx(['nodes', 'docs'], 'readwrite', (t) => {
        for (const n of nodes) t.objectStore('nodes').put(n);
        for (const [id, text] of Object.entries(texts)) t.objectStore('docs').put(text, id);
    });
    announce(nodes.map((n) => n.id));
}

export async function deleteNodes(ids: string[]) {
    await tx(['nodes', 'docs'], 'readwrite', (t) => {
        for (const id of ids) { t.objectStore('nodes').delete(id); t.objectStore('docs').delete(id); }
    });
    announce(ids);
}

export const getMeta = <T>(key: string) => tx(['meta'], 'readonly', (t) => req(t.objectStore('meta').get(key) as IDBRequest<T | undefined>));
export const setMeta = (key: string, value: unknown) => tx(['meta'], 'readwrite', (t) => { t.objectStore('meta').put(value, key); });

// Asks the browser not to evict reviews under storage pressure (granted silently in most browsers).
export const persistStorage = () => navigator.storage?.persist?.().catch(() => false);
