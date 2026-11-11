// Shared state of the connection manager: per-tab registries, storage keys, and request helpers.
// Every other `manager/` module imports from here. This module imports none of them.
// A scope is keyed by collection and member, so a Live grant and the owner's own connection to the
// same collection stay separate.
// `sessions` holds committed replicas only. A replica that is still downloading stays in a private
// staged session until its storage transaction completes.
// Service responses that pass through `request` are untrusted. Each caller validates what it uses.
import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { textBytes, type TreeNode } from '../../tree/model.ts';
import { openThreads } from '../../share/merge.ts';
import { identity, hash } from '../crypto.ts';
import { decode, SyncError, validId, type SignedGrant } from '../protocol.ts';
import { Session, type SessionRecord } from '../session.ts';
import { Transport, type ConnectionStatus } from '../transport.ts';
import { BodyReplica } from '../replica.ts';
import type { RotationPlan } from '../rotation.ts';
import type { LiveLink } from '../live-link.ts';
import { syncDebug, safeError } from '../debug.ts';
import { connectionStates } from '../status.ts';

/**
 * One connection of this device to a collection: the credentials, keys, and local root of one member.
 * Saved in the `meta` store under `scopeKey`. It holds the bearer token, the root data key, and the
 * private identity keys. Only the encrypted recovery file may carry it out of this browser.
 */
export interface Scope {
    collection: string;
    /** Remote document ID of the collection root. */
    root: string;
    /** Local tree node ID that holds the root. */
    localRoot: string;
    kind: 'doc' | 'folder';
    /** Data key of the root document for its current epoch: 32 bytes, base64url. */
    key: string;
    /** Bearer token of this member. The member grant carries its hash. */
    token: string;
    /** Self-signed grant of the collection owner. Its public key verifies every other grant and key update. */
    owner: SignedGrant;
    /** Grant of this device. It equals `owner` on the device that created the collection. */
    member: SignedGrant;
    /** Private signing and key-exchange keys that match `member`. */
    identity: Awaited<ReturnType<typeof identity>>;
    /** Remote IDs of documents that this device disconnected. Polls and materialization skip them. */
    exclusions: string[];
    /** Time of the last complete activity-lease renewal, in milliseconds since the Unix epoch. */
    lastRenewed?: number;
    /** True after explicit enrollment or Sync-link creation. False when the scope exists only for Live links. */
    personalSync?: boolean;
    /** Pending deletion of a Live-only remote document, with the resume cursor that the service returned. */
    cleanup?: { document: string; after?: string };
    /** Remote IDs of the documents in a partial recovery. Such a scope does not cover the whole folder. */
    selection?: string[];
    /** The Live link that this recipient joined through. Owner and paired scopes have none. */
    live?: LiveLink;
    /** The address that the recipient opened, kept so that the recipient can forward the link. */
    liveUrl?: string;
    /** Device names by member ID. Each device supplies its own name during pairing. */
    clientNames?: Record<string, string>;
}
/**
 * Per-tab registries. `sessions` maps a local node ID to the session of its committed replica.
 * `scopes` maps a scope ID to its saved scope. `drivers` maps a scope ID to its one transport.
 */
export const sessions = new Map<string, Session>(),
    scopes = new Map<string, Scope>(),
    drivers = new Map<string, Transport>();
/** The session that prepared each uploaded checkpoint, by operation ID. It routes the acknowledgement. */
export const preparing = new Map<string, Session>();
/** The lease renewal in progress for each scope ID. Concurrent callers share it. */
export const renewing = new Map<string, Promise<void>>();
/** The saved key-rotation plan of each scope ID. While a plan exists, the scope shows `attention`. */
export const rotations = new Map<string, RotationPlan>();
/** The key-rotation run in progress in this tab for each scope ID. */
export const rotationTasks = new Map<string, Promise<void>>();
/** The Live-only cleanup run in progress in this tab for each scope ID. */
export const cleanupTasks = new Map<string, Promise<void>>();
/** The latest folder materialization of each scope ID. The next one waits for it. */
export const materializations = new Map<string, Promise<void>>();
/** The `materializationVersion` that the local tree of each scope ID matches. */
export const materializedViews = new Map<string, string>();
/** The enrollment in progress for each newly created local node ID. */
export const creations = new Map<string, Promise<void>>();
/** `meta` store key of a saved scope. It is also the scope ID in every registry. */
export const scopeKey = (collection: string, member: string) => `sync:scope:${collection}:${member}`;
export const scopeId = (scope: Scope) => scopeKey(scope.collection, scope.member.grant.member);
/** `meta` store key of the saved key-rotation plan of a scope. Rotation also uses it as its Web Locks name. */
export const rotationKey = (scope: Scope) => `sync:rotation:${scopeId(scope)}`;
/** Members and Live invitations whose revocation finished. Later rotations wrap no keys for them. */
export interface RevokedAccess {
    members: string[];
    invitations: string[];
}
/** `meta` store key of the `RevokedAccess` record of a scope. */
export const revokedAccessKey = (scope: Scope) => `sync:revoked-access:${scopeId(scope)}`;
/**
 * Storage adapter for the key-rotation plan of a scope.
 * `commit` writes only when the stored revision equals `expected`, and throws `ReplicaConflict` otherwise.
 * A `null` value clears the plan and, in the same write, adds its target to `RevokedAccess`.
 * `commit` also mirrors the result in `rotations`.
 */
export const rotationRepository = (scope: Scope) => ({
    get: () => db.getMeta<RotationPlan | null>(rotationKey(scope)),
    commit: async (expected: number | null, value: RotationPlan | null) => {
        const extras: Record<string, unknown> = {};
        if (!value) {
            const plan = await db.getMeta<RotationPlan>(rotationKey(scope));
            if (plan) {
                const previous = (await db.getMeta<RevokedAccess>(revokedAccessKey(scope))) ?? {
                    members: [],
                    invitations: [],
                };
                extras[revokedAccessKey(scope)] =
                    'member' in plan.target
                        ? { ...previous, members: [...new Set([...previous.members, plan.target.member])] }
                        : { ...previous, invitations: [...new Set([...previous.invitations, plan.target.invitation])] };
            }
        }
        await db.commitMeta(rotationKey(scope), expected, value, extras);
        if (value) rotations.set(scopeId(scope), value);
        else rotations.delete(scopeId(scope));
    },
});
/** True when the node is bound to this scope: same collection and same member. */
export const belongs = (node: TreeNode, scope: Scope) =>
    node.connection?.collection === scope.collection && node.connection.member === scope.member.grant.member;
export const driverFor = (scope?: Scope) => (scope ? drivers.get(scopeId(scope)) : undefined);
export const errorText = (error: unknown) => (error as Error)?.message ?? String(error);
/** The loaded scope of a connected node. Returns `undefined` for an unconnected node or an unloaded scope. */
export const rootFor = (node: TreeNode) =>
    node.connection ? scopes.get(scopeKey(node.connection.collection, node.connection.member)) : undefined;
/**
 * Storage adapter for the session of local node `id`.
 * `commit` saves the replica record, the tree node, and the Markdown projection in one transaction.
 * It throws `ReplicaConflict` when the stored revision or binding differs, and an `Error` when the
 * node no longer exists. After the write it updates the in-memory node and notifies the app.
 * `remote` marks a commit that did not come from a local edit. Only such a commit can mark the
 * document as changed since the last review.
 */
export const repository = (id: string) => ({
    get: () => db.getSide<SessionRecord>('sync', id),
    commit: async (expected: number | null, record: SessionRecord, remote = false) => {
        const node = app.node(id);
        if (!node) throw new Error('Connection was removed');
        // Shared metadata is document content that any editor can write. Each field below passes a
        // type check before it reaches the tree node.
        const staged = new BodyReplica(decode(record.body)),
            meta = staged.doc.getMap('meta').toJSON(),
            body = staged.text();
        staged.destroy();
        const previous = node.kind === 'doc' ? await db.getText(id) : record.projection;
        const extras: Record<string, unknown> = {};
        // A signed acknowledgement from this participant for exactly this body clears the review
        // marker, including an acknowledgement that another device of the participant made.
        const bodyHash = await hash(new TextEncoder().encode(body));
        const reviewed = Object.values(record.reviews)
            .flatMap((packet) => packet.operations)
            .some(
                (op) =>
                    op.operation.kind === 'ack' &&
                    op.operation.participant === record.member.grant.participant &&
                    op.operation.value === bodyHash,
            );
        // Comments and line endings do not count as an outside change. The first fill of an empty
        // document does not count either.
        const changed =
            remote &&
            previous !== undefined &&
            previous !== '' &&
            app.bodyOf(previous) !== app.bodyOf(record.projection);
        // A protocol-only commit keeps the document timestamp.
        const searchableChanged =
            previous !== record.projection || (typeof meta?.name === 'string' && meta.name !== node.name);
        const updated = {
            ...node,
            ...(changed && !node.pending ? { pending: true as const } : {}),
            ...(typeof meta?.name === 'string' ? { name: meta.name } : {}),
            // Local file/URL identities must survive enrollment and later receipts.
            // The portable UUID in Sync metadata is not the local source location.
            ...(validId(meta?.origin) && (!node.originId || validId(node.originId)) ? { originId: meta.origin } : {}),
            ...(meta.readingWidth === 'full' || typeof meta.readingWidth === 'number'
                ? { readingWidth: meta.readingWidth as 'full' | number }
                : {}),
            // A sync commit never writes to a linked file. The node shows unsaved disk changes instead.
            ...(node.handle && previous !== undefined && previous !== record.projection ? { dirty: true } : {}),
            size: textBytes(record.projection),
            threads: openThreads(record.projection),
            updatedAt: searchableChanged ? Date.now() : node.updatedAt,
        };
        // Keep the text from before the first unreviewed change as the review baseline.
        if (changed && !node.pending) extras[`base:${id}`] = previous;
        if (reviewed) {
            delete updated.pending;
            delete extras[`base:${id}`];
        }
        await db.commitReplica(
            id,
            expected,
            record,
            [updated],
            node.kind === 'doc' ? { [id]: record.projection } : {},
            {},
            extras,
            reviewed ? [`base:${id}`] : [],
            record.binding,
        );
        Object.assign(node, updated);
        connectionStates.restore([node]);
        if (reviewed) delete node.pending;
        app.notify();
    },
});
/** Registers the scope in this tab and saves it in the `meta` store, credentials included. */
export async function saveScope(scope: Scope) {
    scopes.set(scopeId(scope), scope);
    await db.setMeta(scopeId(scope), scope);
}
/**
 * Sends one JSON `POST /sync/<action>` request to the origin of the page and returns the parsed response.
 * The token travels only in the `Authorization` header. The request sends no cookies and no referrer,
 * skips the HTTP cache, and rejects redirects.
 * Throws `SyncError` with the service code and HTTP status for a non-2xx response. Network errors,
 * aborts, and JSON parse errors propagate unchanged.
 * The result is untrusted service input without a size or shape check. The caller validates it.
 * Diagnostics record the action, status, and duration only.
 */
export async function request(action: string, body: Record<string, unknown>, token: string, signal?: AbortSignal) {
    const started = performance.now();
    try {
        const response = await fetch(`/sync/${action}`, {
            method: 'POST',
            headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
            cache: 'no-store',
            credentials: 'omit',
            referrerPolicy: 'no-referrer',
            redirect: 'error',
            signal,
        });
        syncDebug.record('request.finished', {
            action,
            status: response.status,
            duration: Math.round(performance.now() - started),
        });
        const result = await response.json();
        if (!response.ok) throw new SyncError(result.error ?? 'sync-request-failed', response.status);
        return result;
    } catch (error) {
        syncDebug.record('request.failed', {
            action,
            duration: Math.round(performance.now() - started),
            ...safeError(error),
        });
        throw error;
    }
}
/** `request` as the member of a scope: adds the collection and member IDs, and sends the token of the scope. */
export const scoped = (scope: Scope, action: string, values: Record<string, unknown> = {}, signal?: AbortSignal) =>
    request(
        action,
        { collection: scope.collection, member: scope.member.grant.member, ...values },
        scope.token,
        signal,
    );
/**
 * Publishes the connection status of a scope to the status registry, to each bound tree node, and to
 * the viewer. A saved key-rotation plan replaces every status with `attention`.
 * Only `ended` reaches storage, as a best-effort write. The other states live in this tab.
 */
export function phase(scope: Scope, state: ConnectionStatus) {
    syncDebug.record('transport.status', { state });
    if (rotations.has(scopeId(scope))) state = 'attention';
    connectionStates.set({ collection: scope.collection, member: scope.member.grant.member }, state);
    for (const node of app.state.nodes) if (belongs(node, scope)) node.connection!.phase = state;
    if (state === 'ended') void db.markConnectionEnded(scope.collection, scope.member.grant.member).catch(() => {});
    viewer.refreshConnection();
    app.notify();
}
/** Opens the Copy link dialog for document `id` with Live selected. The share module loads on demand. */
export async function live(id: string) {
    return (await import('../../share/share.ts')).copyLinkDialog(id, 'live');
}
/**
 * A Live link that this owner created, saved in the `meta` store under `sync:links:<scope ID>`.
 * The owner keeps both secrets to revoke the link and to encrypt later key updates for it.
 */
export interface OwnedLink {
    /** Invitation ID on the service. */
    id: string;
    /** Remote ID of the shared document. */
    document: string;
    /** Expiry of the invitation, in seconds since the Unix epoch. */
    expires: number;
    /** Bearer token for the invitation endpoints. */
    secret: string;
    /** Key that encrypts key updates for this link. It differs from `secret`, and no request carries it. */
    wrappingKey: string;
    permission?: 'view' | 'comment' | 'edit';
    label?: string;
}
