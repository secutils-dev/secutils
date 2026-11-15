// Sidebar presentation separates configured Sync from connection activity. Healthy descendants
// inherit their folder's badge; enrollment failures and independent Live links remain explicit.
import { ancestors, type TreeNode } from '../tree/model.ts';

/** The outermost folder with the same personal-Sync connection, or undefined for a connection root. */
export function syncedAncestor(nodes: TreeNode[], node: TreeNode) {
    const binding = node.connection;
    if (binding?.kind !== 'sync') return;
    return ancestors(nodes, node.id).find(
        (parent) =>
            parent.kind === 'folder' &&
            parent.connection?.kind === 'sync' &&
            parent.connection.collection === binding.collection &&
            parent.connection.member === binding.member,
    );
}

/** Whether a row needs a separate connection control: roots, owned/received Live links, or row-local issues. */
export function connectionBadgeVisible(nodes: TreeNode[], node: TreeNode) {
    if (node.syncPending) return true;
    if (!node.connection) return false;
    if (node.connection.kind === 'live' || node.connection.liveShared) return true;
    const root = syncedAncestor(nodes, node);
    return !root || (node.connection.phase === 'ended' && root.connection?.phase !== 'ended');
}

/** Whether the existing file/folder glyph gets the static configured-Sync mark. */
export const hasSyncGlyph = (node: TreeNode) => node.connection?.kind === 'sync' && node.connection.phase !== 'ended';
