// Folder inheritance and local-copy placement. This module only plans tree changes: it never
// modifies document text, connection keys, or the caller's node objects.
import { ancestors, newId, nextOrder, type TreeNode } from '../tree/model.ts';

/** Name of the browser-local container used when legacy copies cannot safely join folder Sync. */
export const LOCAL_COPIES_FOLDER = 'Local copies';

/** Whether a node remains below an active Sync folder after `detached` nodes lose their bindings. */
export function insideSyncedFolder(nodes: TreeNode[], id: string, detached = new Set<string>()) {
    return ancestors(nodes, id).some(
        (node) =>
            !detached.has(node.id) &&
            node.kind === 'folder' &&
            node.connection?.kind === 'sync' &&
            node.connection.phase !== 'ended',
    );
}

/** Plans parent-first relocation of complete local subtrees. `container` must have no connection or saved replica. */
export function localCopyPlan(nodes: TreeNode[], ids: Set<string>, container?: TreeNode) {
    const roots = nodes.filter(
        (node) => ids.has(node.id) && !ancestors(nodes, node.id).some((parent) => ids.has(parent.id)),
    );
    if (!roots.length) return [];
    const names = new Set(nodes.filter((node) => node.parentId === null).map((node) => node.name));
    let name = LOCAL_COPIES_FOLDER,
        suffix = 2;
    while (names.has(name)) name = `${LOCAL_COPIES_FOLDER} (${suffix++})`;
    const folder: TreeNode = container ?? {
        id: newId(),
        parentId: null,
        kind: 'folder',
        name,
        order: nextOrder(nodes, null),
        createdAt: Date.now(),
        updatedAt: Date.now(),
        open: false,
    };
    let order = nextOrder(nodes, folder.id);
    return [...(container ? [] : [folder]), ...roots.map((node) => ({ ...node, parentId: folder.id, order: order++ }))];
}

/** Recognizes legacy local-only intent conservatively; new creates carry explicit durable enrollment instead. */
export function legacyLocalCopy(node: TreeNode, excluded: boolean) {
    return (node.kind === 'doc' && !node.originId) || (!!node.originId && node.originId !== node.id && excluded);
}
