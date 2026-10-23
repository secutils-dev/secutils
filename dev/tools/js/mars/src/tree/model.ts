// Pure tree helpers over the flat node list stored in IndexedDB.
export interface TreeNode {
    id: string;
    parentId: string | null;
    kind: 'folder' | 'doc';
    name: string;
    order: number;
    createdAt: number;
    updatedAt: number;
    // Stable across shares, so a copy that comes back can merge with the original.
    originId?: string;
    open?: boolean;
    // Linked file (Chromium File System Access): handle, its lastModified at the last
    // read/write, and whether browser edits are newer than the file.
    handle?: FileSystemFileHandle;
    stamp?: number;
    dirty?: boolean;
    threads?: number;
    // UTF-8 bytes of the stored text, for the sidebar footer.
    size?: number;
}

export const newId = () => crypto.randomUUID();
const utf8 = new TextEncoder();
export const textBytes = (text: string) => utf8.encode(text).length;

const byOrder = (a: TreeNode, b: TreeNode) =>
    a.order - b.order || (a.kind === b.kind ? 0 : a.kind === 'folder' ? -1 : 1) || a.name.localeCompare(b.name);

// Nodes whose parent is gone (deleted in another tab, say) show at the top level.
export function childrenOf(nodes: TreeNode[], parentId: string | null) {
    if (parentId !== null) return nodes.filter((n) => n.parentId === parentId).sort(byOrder);
    const ids = new Set(nodes.map((n) => n.id));
    return nodes.filter((n) => n.parentId === null || !ids.has(n.parentId)).sort(byOrder);
}

export function descendants(nodes: TreeNode[], id: string): TreeNode[] {
    const out: TreeNode[] = [];
    for (const child of nodes.filter((n) => n.parentId === id)) out.push(child, ...descendants(nodes, child.id));
    return out;
}

export function ancestors(nodes: TreeNode[], id: string | null): TreeNode[] {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const out: TreeNode[] = [];
    for (let n = id ? byId.get(id) : undefined; n?.parentId; n = byId.get(n.parentId)) {
        const parent = byId.get(n.parentId);
        if (!parent || out.includes(parent)) break;
        out.unshift(parent);
    }
    return out;
}

// A folder cannot move into itself or one of its descendants.
export const canMove = (nodes: TreeNode[], id: string, parentId: string | null) =>
    parentId === null || (parentId !== id && !descendants(nodes, id).some((n) => n.id === parentId));

// Moves `id` under `parentId`, before `beforeId` (or last). Returns the nodes whose
// parent or order changed; sibling orders are renumbered 0..n.
export function moveNode(nodes: TreeNode[], id: string, parentId: string | null, beforeId: string | null = null): TreeNode[] {
    const node = nodes.find((n) => n.id === id);
    if (!node || !canMove(nodes, id, parentId)) return [];
    const siblings = childrenOf(nodes, parentId).filter((n) => n.id !== id);
    const at = beforeId ? siblings.findIndex((n) => n.id === beforeId) : -1;
    siblings.splice(at < 0 ? siblings.length : at, 0, node);
    const changed: TreeNode[] = [];
    siblings.forEach((n, order) => {
        const parent = n === node ? parentId : n.parentId;
        if (n.order !== order || n.parentId !== parent) {
            n.order = order;
            n.parentId = parent;
            changed.push(n);
        }
    });
    return changed;
}

export const nextOrder = (nodes: TreeNode[], parentId: string | null) =>
    Math.max(-1, ...nodes.filter((n) => n.parentId === parentId).map((n) => n.order)) + 1;

// File-system path segments for export, with sibling names made unique ("a", "a (2)").
export function exportPaths(nodes: TreeNode[], rootId: string | null): Map<string, string> {
    const out = new Map<string, string>();
    const walk = (parentId: string | null, prefix: string) => {
        const used = new Set<string>();
        for (const n of childrenOf(nodes, parentId)) {
            const base = n.name.replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '-').replace(/^[.\s]+|[.\s]+$/g, '').slice(0, 120) || 'Untitled';
            const ext = n.kind === 'doc' ? '.md' : '';
            let name = base, i = 2;
            while (used.has(`${name}${ext}`.toLowerCase())) name = `${base} (${i++})`;
            used.add(`${name}${ext}`.toLowerCase());
            out.set(n.id, `${prefix}${name}${ext}`);
            if (n.kind === 'folder') walk(n.id, `${prefix}${name}/`);
        }
    };
    walk(rootId, '');
    return out;
}
