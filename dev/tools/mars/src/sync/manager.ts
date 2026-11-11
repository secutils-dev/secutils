// Public API of the connection manager for Sync (own devices) and Live links (other people).
// `index.ts` loads this module on demand, so Yjs, crypto, and the dialogs stay out of the startup path.
// Only the names below are public. The `manager/` modules import each other directly.
//
// - `manager/state.ts`: per-tab registries (scopes, sessions, transports), storage keys, request helpers.
// - `manager/engine.ts`: startup, transports, received checkpoints, folder materialization, lease renewal,
//   and local edits of connected documents.
// - `manager/enrollment.ts`: creates a collection from a local document or folder.
// - `manager/pairing.ts`: creates and receives Sync links, and runs the code confirmation.
// - `manager/live-links.ts`: creates, receives, forwards, and saves Live links, and deletes Live-only copies.
// - `manager/access.ts`: stop sync, deletion, moves, revocation, and key rotation.
// - `manager/recovery.ts`: encrypted recovery export and restore.
// - `manager/dialogs.ts`: the Sync dialog and the user-visible error text.
export { live } from './manager/state.ts';
export {
    initialize,
    refresh,
    present,
    snapshot,
    persist,
    metadata,
    created,
    opened,
    undo,
    markReviewed,
    undoReview,
    sharedWidth,
} from './manager/engine.ts';
export { receiveLocation } from './manager/pairing.ts';
export { liveLinks, liveLinkInfo, forwardLiveLink, createLiveLink, saveLive } from './manager/live-links.ts';
export { unlink, remove, deletionInfo, beforeMove } from './manager/access.ts';
export { exportRecovery, restoreRecovery } from './manager/recovery.ts';
export { enroll, details } from './manager/dialogs.ts';
