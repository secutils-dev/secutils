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

export interface Scope {
    collection: string;
    root: string;
    localRoot: string;
    kind: 'doc' | 'folder';
    key: string;
    token: string;
    owner: SignedGrant;
    member: SignedGrant;
    identity: Awaited<ReturnType<typeof identity>>;
    exclusions: string[];
    lastRenewed?: number;
    personalSync?: boolean;
    cleanup?: { document: string; after?: string };
    selection?: string[];
    live?: LiveLink;
    liveUrl?: string;
    clientNames?: Record<string, string>;
}
export const sessions = new Map<string, Session>(),
    scopes = new Map<string, Scope>(),
    drivers = new Map<string, Transport>();
export const preparing = new Map<string, Session>();
export const renewing = new Map<string, Promise<void>>();
export const rotations = new Map<string, RotationPlan>();
export const rotationTasks = new Map<string, Promise<void>>();
export const cleanupTasks = new Map<string, Promise<void>>();
export const materializations = new Map<string, Promise<void>>();
export const materializedViews = new Map<string, string>();
export const creations = new Map<string, Promise<void>>();
export const scopeKey = (collection: string, member: string) => `sync:scope:${collection}:${member}`;
export const scopeId = (scope: Scope) => scopeKey(scope.collection, scope.member.grant.member);
export const rotationKey = (scope: Scope) => `sync:rotation:${scopeId(scope)}`;
export interface RevokedAccess {
    members: string[];
    invitations: string[];
}
export const revokedAccessKey = (scope: Scope) => `sync:revoked-access:${scopeId(scope)}`;
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
export const belongs = (node: TreeNode, scope: Scope) =>
    node.connection?.collection === scope.collection && node.connection.member === scope.member.grant.member;
export const driverFor = (scope?: Scope) => (scope ? drivers.get(scopeId(scope)) : undefined);
export const errorText = (error: unknown) => (error as Error)?.message ?? String(error);
export const rootFor = (node: TreeNode) =>
    node.connection ? scopes.get(scopeKey(node.connection.collection, node.connection.member)) : undefined;
export const repository = (id: string) => ({
    get: () => db.getSide<SessionRecord>('sync', id),
    commit: async (expected: number | null, record: SessionRecord, remote = false) => {
        const node = app.node(id);
        if (!node) throw new Error('Connection was removed');
        const staged = new BodyReplica(decode(record.body)),
            meta = staged.doc.getMap('meta').toJSON(),
            body = staged.text();
        staged.destroy();
        const previous = node.kind === 'doc' ? await db.getText(id) : record.projection;
        const extras: Record<string, unknown> = {};
        const bodyHash = await hash(new TextEncoder().encode(body));
        const reviewed = Object.values(record.reviews)
            .flatMap((packet) => packet.operations)
            .some(
                (op) =>
                    op.operation.kind === 'ack' &&
                    op.operation.participant === record.member.grant.participant &&
                    op.operation.value === bodyHash,
            );
        const changed =
            remote &&
            previous !== undefined &&
            previous !== '' &&
            app.bodyOf(previous) !== app.bodyOf(record.projection);
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
            ...(node.handle && previous !== undefined && previous !== record.projection ? { dirty: true } : {}),
            size: textBytes(record.projection),
            threads: openThreads(record.projection),
            updatedAt: searchableChanged ? Date.now() : node.updatedAt,
        };
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
export async function saveScope(scope: Scope) {
    scopes.set(scopeId(scope), scope);
    await db.setMeta(scopeId(scope), scope);
}
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
export const scoped = (scope: Scope, action: string, values: Record<string, unknown> = {}, signal?: AbortSignal) =>
    request(
        action,
        { collection: scope.collection, member: scope.member.grant.member, ...values },
        scope.token,
        signal,
    );
export function phase(scope: Scope, state: ConnectionStatus) {
    syncDebug.record('transport.status', { state });
    if (rotations.has(scopeId(scope))) state = 'attention';
    connectionStates.set({ collection: scope.collection, member: scope.member.grant.member }, state);
    for (const node of app.state.nodes) if (belongs(node, scope)) node.connection!.phase = state;
    if (state === 'ended') void db.markConnectionEnded(scope.collection, scope.member.grant.member).catch(() => {});
    viewer.refreshConnection();
    app.notify();
}
export async function live(id: string) {
    return (await import('../../share/share.ts')).copyLinkDialog(id, 'live');
}
export interface OwnedLink {
    id: string;
    document: string;
    expires: number;
    secret: string;
    wrappingKey: string;
    permission?: 'view' | 'comment' | 'edit';
    label?: string;
}
