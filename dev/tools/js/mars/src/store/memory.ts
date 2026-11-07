// The temporary session never opens IndexedDB or broadcasts document contents to other tabs.
export type Store = 'nodes' | 'docs' | 'meta';

export class MemoryStore {
    private stores: Record<Store, Map<string, unknown>> = { nodes: new Map(), docs: new Map(), meta: new Map() };

    get<T>(store: Store, key: string): T | undefined {
        return structuredClone(this.stores[store].get(key)) as T | undefined;
    }

    put(store: Store, key: string, value: unknown) {
        this.stores[store].set(key, structuredClone(value));
    }

    all<T>(store: Store, prefix = ''): T[] {
        return [...this.stores[store]].filter(([key]) => key.startsWith(prefix)).map(([, value]) => structuredClone(value) as T);
    }

    delete(store: Store, key: string) { this.stores[store].delete(key); }

    clearPrefix(store: Store, prefix: string) {
        for (const key of this.stores[store].keys()) if (key.startsWith(prefix)) this.stores[store].delete(key);
    }
}
