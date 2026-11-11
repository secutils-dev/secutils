// A manifest entry is publishable only with a complete, registered folder chain.
// Never repair an incomplete/invalid chain by briefly displaying it at the root.
// Manifest entries come from the shared replica, which any editor can write.
// The service directory names the registered IDs. These functions are pure: no storage, network, or DOM access.
import type { ManifestNode } from './session.ts';
import type { SyncDirectory } from './protocol.ts';

/**
 * Version key of one materialized view. It changes with the root, the manifest state, the exclusions, or the ID, kind,
 * or deleted flag of a directory item. An equal key means that materialization can be skipped.
 * @param body Yjs state of the root replica in base64url.
 */
export const materializationVersion = (root: string, body: string, exclusions: string[], directory: SyncDirectory) =>
    JSON.stringify([root, body, exclusions, directory.documents.map((item) => [item.id, item.kind, item.deleted])]);

/**
 * Selects the manifest entries that are safe to publish, each parent before its children. An entry qualifies when the
 * directory registers its ID with the same kind and not deleted, and the ID is not excluded. Its name is a string of at
 * most 256 characters. A document carries a 32-byte key (43 base64url characters). Each ancestor up to `root` must be a
 * qualifying folder. A cycle or a missing parent drops the entry.
 * @param root Remote ID of the scope root. A `null` parent means the root.
 * @param registered Directory items by remote ID.
 */
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
