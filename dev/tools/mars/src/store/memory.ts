// In-memory backend of a temporary session, with the same three stores as the IndexedDB database.
// The temporary session never opens IndexedDB or broadcasts document contents to other tabs.
//
// Each value is cloned on write and on read (`structuredClone`), as IndexedDB does. A caller
// cannot change stored data through a reference, and a value that cannot be cloned throws
// `DataCloneError`. Every operation is synchronous, and the data ends with the tab.

/** The object stores of the `mars` database. */
export type Store = 'nodes' | 'docs' | 'meta';

/** A key-value store for each `Store` name. Keys keep their insertion order. */
export class MemoryStore {
    private stores: Record<Store, Map<string, unknown>> = { nodes: new Map(), docs: new Map(), meta: new Map() };

    /** A copy of the stored value, or undefined. */
    get<T>(store: Store, key: string): T | undefined {
        return structuredClone(this.stores[store].get(key)) as T | undefined;
    }

    put(store: Store, key: string, value: unknown) {
        this.stores[store].set(key, structuredClone(value));
    }

    /** Copies of the values whose key starts with `prefix`, in insertion order. */
    all<T>(store: Store, prefix = ''): T[] {
        return [...this.stores[store]]
            .filter(([key]) => key.startsWith(prefix))
            .map(([, value]) => structuredClone(value) as T);
    }

    delete(store: Store, key: string) {
        this.stores[store].delete(key);
    }

    /** Deletes each entry whose key starts with `prefix`. */
    clearPrefix(store: Store, prefix: string) {
        for (const key of this.stores[store].keys()) if (key.startsWith(prefix)) this.stores[store].delete(key);
    }
}
