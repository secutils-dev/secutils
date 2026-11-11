// Lightweight integration surface. Yjs, crypto, and connection dialogs initialize only
// when a user connects content or opens an existing connected replica.
// Each function below forwards to the connection manager (`manager.ts`) after one shared dynamic import.
// A returned promise rejects when the import fails or when the manager function fails.
import type { TreeNode } from '../tree/model.ts';
import type { Thread } from '../share/merge.ts';
export { connectionStates } from './status.ts';
// The manager module loads on the first call. Later calls share that import.
let module: Promise<typeof import('./manager.ts')> | null = null;
const api = () => (module ??= import('./manager.ts'));
/** User-visible text of each connection phase. */
export const labels = {
    waiting: 'Saved locally · waiting to sync',
    working: 'Syncing',
    current: 'Up to date',
    offline: 'Offline · changes saved locally',
    attention: 'Needs attention',
    ended: 'Local copy · connection ended',
};
/** Status text of a node. Empty for a node without a connection. A missing phase reads as `waiting`. */
export const label = (node?: TreeNode) => (node?.connection ? labels[node.connection.phase ?? 'waiting'] : '');
// Forwards to the connection manager. The contract of each function is at its definition in `manager/`.
export const enroll = (id: string) => api().then((m) => m.enroll(id));
export const details = (id: string) => api().then((m) => m.details(id));
export const persist = (node: TreeNode, text: string) => api().then((m) => m.persist(node, text));
export const opened = (id: string) => api().then((m) => m.opened(id));
export const unlink = (id: string) => api().then((m) => m.unlink(id));
export const beforeMove = (id: string, parent: string | null) => api().then((m) => m.beforeMove(id, parent));
export const created = (id: string) => api().then((m) => m.created(id));
export const metadata = (id: string) => api().then((m) => m.metadata(id));
export const initialize = () => api().then((m) => m.initialize());
export const refresh = (ids: string[]) => api().then((m) => m.refresh(ids));
export const present = (id: string) => api().then((m) => m.present(id));
export const snapshot = (node: TreeNode, text: string) => api().then((m) => m.snapshot(node, text));
/**
 * True when the URL fragment has a `sync`, `invite`, or `live` parameter. It is synchronous and does not load the
 * manager. It throws for an invalid URL.
 */
export const isConnectionLocation = (url: string) => {
    const params = new URLSearchParams(new URL(url).hash.slice(1));
    return params.has('sync') || params.has('invite') || params.has('live');
};
/** Handles a connection link. Resolves with `false`, without a manager import, for any other URL. */
export const receiveLocation = (url: string) =>
    isConnectionLocation(url) ? api().then((m) => m.receiveLocation(url)) : Promise.resolve(false);
export const undo = (id: string, redo = false) => api().then((m) => m.undo(id, redo));
export const live = (id: string) => api().then((m) => m.live(id));
export const liveLinks = (id: string) => api().then((m) => m.liveLinks(id));
export const createLiveLink = (
    id: string,
    permission: 'view' | 'comment' | 'edit',
    ttl: number,
    format: 'short' | 'full',
) => api().then((m) => m.createLiveLink(id, permission, ttl, format));
export const liveLinkInfo = (id: string) => api().then((m) => m.liveLinkInfo(id));
export const forwardLiveLink = (id: string) => api().then((m) => m.forwardLiveLink(id));
export const saveLive = (id: string) => api().then((m) => m.saveLive(id));
export const remove = (id: string, remote = false) => api().then((m) => m.remove(id, remote));
export const deletionInfo = (ids: string[]) => api().then((m) => m.deletionInfo(ids));
export const restoreRecovery = (file?: File) => api().then((m) => m.restoreRecovery(file));
export const exportRecovery = (ids: string[], password: string) => api().then((m) => m.exportRecovery(ids, password));
export const markReviewed = (id: string) => api().then((m) => m.markReviewed(id));
export const undoReview = (id: string, before: Thread[], after: Thread[]) =>
    api().then((m) => m.undoReview(id, before, after));
export const sharedWidth = (id: string, width: number | 'full') => api().then((m) => m.sharedWidth(id, width));
