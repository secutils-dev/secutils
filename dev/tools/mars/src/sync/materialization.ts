// A manifest entry is publishable only with a complete, registered folder chain.
// Never repair an incomplete/invalid chain by briefly displaying it at the root.
import type { ManifestNode } from './session.ts';
import type { SyncDirectory } from './protocol.ts';

export const materializationVersion = (root: string, body: string, exclusions: string[], directory: SyncDirectory) =>
    JSON.stringify([root, body, exclusions, directory.documents.map((item) => [item.id, item.kind, item.deleted])]);

export function materializable(
    root: string,
    values: ManifestNode[],
    registered: Map<string, { kind: string; deleted: boolean }>,
    exclusions: string[],
) {
    const entries = new Map(
        values
            .filter(
                (item) =>
                    item.id !== root &&
                    !exclusions.includes(item.id) &&
                    registered.get(item.id)?.kind === item.kind &&
                    !registered.get(item.id)?.deleted &&
                    ['doc', 'folder'].includes(item.kind) &&
                    typeof item.name === 'string' &&
                    item.name.length <= 256 &&
                    (item.kind !== 'doc' || (typeof item.key === 'string' && /^[A-Za-z0-9_-]{43}$/.test(item.key))),
            )
            .map((item) => [item.id, { ...item, parent: item.parent ?? root }]),
    );
    const result: ManifestNode[] = [],
        ready = new Set([root]);
    const visit = (item: ManifestNode, chain = new Set<string>()): boolean => {
        if (ready.has(item.id)) return true;
        if (chain.has(item.id)) return false;
        chain.add(item.id);
        if (item.parent !== root) {
            const parent = entries.get(item.parent!);
            if (!parent || parent.kind !== 'folder' || !visit(parent, chain)) return false;
        }
        ready.add(item.id);
        result.push(item);
        return true;
    };
    for (const item of entries.values()) visit(item);
    return result;
}
