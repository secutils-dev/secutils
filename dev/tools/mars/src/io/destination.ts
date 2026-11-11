import { ancestors, type TreeNode } from '../tree/model.ts';

export const DEFAULT_DESTINATION_KEY = 'defaultIncomingFolder';
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
