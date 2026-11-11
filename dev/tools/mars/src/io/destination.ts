// The default destination of incoming documents (files, links, and URLs).
// Pure: it reads the node list only.
import { ancestors, type TreeNode } from '../tree/model.ts';

/** `meta` key that holds the id of the default incoming folder. */
export const DEFAULT_DESTINATION_KEY = 'defaultIncomingFolder';
/**
 * Returns the folder `id` when it can receive a new document, else undefined.
 * The folder and each ancestor must be local, or connected with `owner` or `edit` permission
 * and a connection that has not ended. A document id, an unknown id, and null return undefined.
 */
export function incomingFolder(nodes: TreeNode[], id: string | null | undefined) {
    const folder = nodes.find((node) => node.id === id && node.kind === 'folder');
    if (!folder) return undefined;
    const editable = [folder, ...ancestors(nodes, folder.id)].every(
        (node) =>
            !node.connection ||
            (node.connection.phase !== 'ended' && ['owner', 'edit'].includes(node.connection.permission)),
    );
    return editable ? folder : undefined;
}
