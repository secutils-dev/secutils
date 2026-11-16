// Incremental enrollment: exact retry-stable registrations, visible pending state, and conservative
// recovery. One Web Lock per collection prevents competing seeds in same-profile tabs. The durable
// job remains until both the node binding and encrypted manifest are saved. Active folder Sync
// reconciles missing descendants automatically; legacy local-only intent stays outside the folder.
import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { ancestors, descendants, type TreeNode } from '../../tree/model.ts';
import { creationIntent, type CreationJob } from '../creation.ts';
import { Session, stagedSession, type SessionRecord } from '../session.ts';
import { validId, validGrant, decode, SyncError } from '../protocol.ts';
import { safeError } from '../debug.ts';
import { verifyGrant } from '../crypto.ts';
import { legacyLocalCopy, localCopyPlan, LOCAL_COPIES_FOLDER } from '../inheritance.ts';
import { initialize } from './engine.ts';
import { scopes, scopeKey, sessions, repository, belongs, driverFor, scoped, type Scope } from './state.ts';

const RETRY_MS = 2000;
const MAX_RETRY_MS = 60000;
let running: Promise<void> | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;
let controller: AbortController | undefined;
let failures = 0;
let active = true;
let retryRequested = false;
const available = () => active && !document.hidden && navigator.onLine;

async function matchingReplica(record: SessionRecord, scope: Scope) {
    return (
        record?.kind === 'connected' &&
        !record.ended &&
        validId(record.binding?.document) &&
        record.binding.collection === scope.collection &&
        record.binding.member === scope.member.grant.member &&
        record.binding.permission === scope.member.grant.permission &&
        record.member?.grant.member === scope.member.grant.member &&
        record.member.grant.publicKey === scope.member.grant.publicKey &&
        record.owner?.grant.publicKey === scope.owner.grant.publicKey &&
        record.token === scope.token &&
        validGrant(record.member) &&
        decode(record.key).length === 32 &&
        !scope.exclusions.includes(record.binding.document) &&
        (await verifyGrant(record.member, scope.owner.grant))
    );
}

/** Pending registrations of this scope, including failed ones. They prevent a false Up to date status. */
export const pendingCreations = (scope: Scope) =>
    app.state.nodes.filter(
        (node) =>
            node.syncPending?.collection === scope.collection && node.syncPending.member === scope.member.grant.member,
    );

/** Queues an explicitly created/moved node. Concurrent calls share the durable job and collection lock. */
export async function created(id: string) {
    await initialize();
    if (!(await db.getSide<CreationJob>('sync-create', id))) {
        const all = await db.allNodes(),
            node = all.find((item) => item.id === id);
        if (!node) return;
        const job = creationIntent(all, node);
        if (!job) return;
        await app.serializePersistence(async () => {
            await db.putNodes([node], {}, { creations: [job] });
            if (app.node(id)) app.node(id)!.syncPending = node.syncPending;
            app.notify();
        });
    }
    resumeCreations();
}

/** Reconciles active folder descendants before registration. Returns true when an editor draft deferred repair. */
async function reconcileFolders(signal: AbortSignal) {
    let deferred = false;
    const run = async () => {
        signal.throwIfAborted();
        const prepared = await db.allNodes();
        const jobs: CreationJob[] = [],
            queued: TreeNode[] = [],
            local = new Set<string>();
        for (const scope of scopes.values()) {
            const root = prepared.find((node) => node.id === scope.localRoot);
            if (
                scope.kind !== 'folder' ||
                scope.selection ||
                scope.live ||
                !root ||
                !belongs(root, scope) ||
                root.connection?.phase === 'ended' ||
                !['owner', 'edit'].includes(scope.member.grant.permission)
            )
                continue;
            const manifest = new Set(
                sessions
                    .get(root.id)
                    ?.manifest()
                    .map((node) => node.id) ?? [],
            );
            for (const node of descendants(prepared, root.id)) {
                if (ancestors(prepared, node.id).some((parent) => local.has(parent.id) && parent.id !== root.id))
                    continue;
                if (node.syncPending) continue;
                if (belongs(node, scope) && node.connection?.phase !== 'ended') {
                    // An old partial setup can retain bindings but lose its manifest. Repair the
                    // existing identities, rather than creating another collection or seed.
                    if (!manifest.has(node.connection!.document)) {
                        const record =
                            node.kind === 'doc' ? await db.getSide<SessionRecord>('sync', node.id) : undefined;
                        if (node.kind === 'doc' && (!record || !(await matchingReplica(record, scope)))) continue;
                        const job: CreationJob = {
                            v: 1,
                            id: node.id,
                            collection: scope.collection,
                            member: scope.member.grant.member,
                            root: scope.root,
                            document: node.connection!.document,
                            key: record?.key ?? scope.key,
                        };
                        node.syncPending = {
                            collection: job.collection,
                            member: job.member,
                            root: job.root,
                            phase: 'waiting',
                        };
                        jobs.push(job);
                        queued.push(node);
                    }
                    continue;
                }
                // A repair must not commit a reader's open block/cell draft as a side effect of polling.
                if (node.id === app.state.currentId && (viewer.isEditing() || !viewer.canPublishConnection())) {
                    deferred = true;
                    continue;
                }
                if (node.connection) {
                    local.add(node.id);
                    continue;
                }
                const record = await db.getSide<SessionRecord>('sync', node.id);
                // Old child folders have no replica or portable identity once disconnected.
                // Keep such subtrees intact outside Sync instead of guessing an identity from their names.
                if (!record && node.kind === 'folder') {
                    local.add(node.id);
                    continue;
                }
                if (!record && legacyLocalCopy(node, scope.exclusions.length > 0)) {
                    local.add(node.id);
                    continue;
                }
                if (record) {
                    let matches = false;
                    try {
                        matches = await matchingReplica(record, scope);
                    } catch {
                        /* Retain malformed state as a local copy. */
                    }
                    if (!matches) {
                        local.add(node.id);
                        continue;
                    }
                }
                const job = creationIntent(prepared, node);
                if (!job) continue;
                if (record) {
                    job.document = record.binding.document;
                    job.key = record.key;
                    job.recovery = true;
                }
                jobs.push(job);
                queued.push(node);
            }
        }
        let container = prepared.find(
            (node) =>
                node.parentId === null &&
                node.kind === 'folder' &&
                node.name === LOCAL_COPIES_FOLDER &&
                !node.connection &&
                !node.syncPending,
        );
        if (container && (await db.getSide('sync', container.id))) container = undefined;
        const moved = localCopyPlan(prepared, local, container);
        if (!queued.length && !moved.length) return;
        await app.serializePersistence(async () => {
            signal.throwIfAborted();
            await db.putNodes([...moved, ...queued], {}, { creations: jobs });
            await app.load();
            if (app.current()) app.state.folderId = app.current()!.parentId;
        });
        if (local.size)
            viewer.toast(
                'Local copies were kept outside synced folders. Their text and saved connection data are retained.',
            );
    };
    if (navigator.locks) await navigator.locks.request('mars:folder-inheritance', { signal }, run);
    else await run();
    return deferred;
}

async function enroll(job: CreationJob, signal: AbortSignal) {
    if (
        job.v !== 1 ||
        ![job.id, job.collection, job.member, job.document, job.root].every(validId) ||
        decode(job.key).length !== 32
    )
        throw new SyncError('invalid-enrollment-job', 400);
    const scope = scopes.get(scopeKey(job.collection, job.member));
    if (!scope) return;
    if (scope.kind !== 'folder' || scope.selection || scope.live)
        throw new SyncError('folder-enrollment-unavailable', 409);
    if (scope.root !== job.root) throw new SyncError('invalid-enrollment-job', 400);
    const all = await db.allNodes(),
        node = all.find((item) => item.id === job.id);
    if (!node) {
        await db.deleteSide('sync-create', job.id);
        return;
    }
    const parent = all.find((item) => item.id === node.parentId);
    if (!node.connection && !parent?.connection && !parent?.syncPending) {
        // A deleted or disconnected parent leaves this file independent. Cancel only enrollment,
        // never the text or any preserved replica that automatic recovery was using.
        delete node.syncPending;
        await app.serializePersistence(async () => {
            await db.putNodes([node], {}, { completeCreations: [node.id] });
            if (app.node(node.id)) delete app.node(node.id)!.syncPending;
            app.notify();
        });
        return;
    }
    if (!parent?.connection || !belongs(parent, scope) || parent.connection.phase === 'ended') return;
    if (scope.exclusions.includes(job.document)) return;
    if (node.connection && (!belongs(node, scope) || node.connection.document !== job.document))
        throw new SyncError('already-connected', 409);
    const savedReplica = !node.connection ? await db.getSide<SessionRecord>('sync', node.id) : undefined;
    if (
        savedReplica &&
        (!job.recovery ||
            savedReplica.binding.document !== job.document ||
            !(await matchingReplica(savedReplica, scope)))
    )
        throw new SyncError('orphan-replica', 409);
    signal.throwIfAborted();
    await scoped(scope, 'document', { document: job.document, kind: node.kind }, signal);
    signal.throwIfAborted();
    if (scope.exclusions.includes(job.document)) return;
    if (!node.connection) {
        const expected = structuredClone(node),
            text = node.kind === 'doc' ? ((await db.getText(node.id)) ?? '') : undefined;
        const previous = await db.getSide<SessionRecord>('sync', node.id);
        const binding = previous?.binding ?? {
            collection: job.collection,
            member: job.member,
            document: job.document,
            root: job.root,
            kind: 'sync' as const,
            epoch: 1,
            permission: scope.member.grant.permission,
            participant: scope.member.grant.participant,
            ownerParticipant: scope.owner.grant.participant,
        };
        let record: SessionRecord | undefined;
        let snapshot: db.UnfinishedSetupSnapshot | undefined;
        if (previous) {
            if (
                !job.recovery ||
                previous.binding.document !== job.document ||
                previous.binding.collection !== job.collection ||
                previous.binding.member !== job.member ||
                previous.ended
            )
                throw new SyncError('orphan-replica', 409);
            snapshot = await db.captureUnfinishedSetup([node.id]);
        }
        if (node.kind === 'doc') {
            const staged = stagedSession(
                previous ??
                    Session.seed(
                        binding,
                        text ?? '',
                        scope.identity,
                        scope.token,
                        job.key,
                        scope.owner,
                        node.name,
                        scope.member,
                    ),
            );
            try {
                if (previous) {
                    await staged.change(previous.projection, text ?? '');
                } else await staged.initial();
                await staged.metadataChanged({
                    name: node.name,
                    origin: validId(node.originId) ? node.originId : node.id,
                    ...(node.readingWidth !== undefined ? { readingWidth: node.readingWidth } : {}),
                });
                record = staged.record;
            } finally {
                staged.destroy();
            }
        }
        const connected: TreeNode = {
            ...node,
            connection: { ...binding, phase: 'waiting' },
            ...(node.handle ? { manualFileSave: true } : {}),
        };
        await app.serializePersistence(async () => {
            // Registration can outlive a tree move or a disconnection. Check placement in storage again.
            const current = await db.allNodes(),
                currentParent = current.find((item) => item.id === node.parentId);
            if (!currentParent || !belongs(currentParent, scope))
                throw new db.ReplicaConflict('metadata-changed', 'parentId');
            await db.enrollConnected(
                [expected],
                text === undefined ? {} : { [node.id]: text },
                [connected],
                record ? { [node.id]: record } : {},
                {},
                { incremental: true, previous: snapshot },
            );
            const displayed = app.node(node.id);
            if (displayed) {
                displayed.connection = connected.connection;
                if (connected.manualFileSave) displayed.manualFileSave = true;
            }
            if (record) {
                sessions.get(node.id)?.destroy();
                sessions.set(node.id, new Session(record, repository(node.id)));
            }
            app.notify();
        });
    }
    // Re-read the node and parent, not the objects captured before registration.
    const current = (await db.allNodes()).find((item) => item.id === job.id);
    const currentParent = current ? (await db.allNodes()).find((item) => item.id === current.parentId) : undefined;
    if (
        !current?.connection ||
        !currentParent?.connection ||
        !belongs(current, scope) ||
        !belongs(currentParent, scope)
    )
        return;
    const root = sessions.get(scope.localRoot);
    if (!root) throw new Error('The folder replica is not ready.');
    await root.editManifest([
        {
            id: job.document,
            kind: current.kind,
            parent: currentParent.connection.document,
            name: current.name,
            order: current.order,
            ...(current.kind === 'doc'
                ? {
                      key: job.key,
                      origin: validId(current.originId) ? current.originId : current.id,
                      epoch: current.connection.epoch,
                  }
                : {}),
        },
    ]);
    await app.serializePersistence(async () => {
        const latest = (await db.allNodes()).find((item) => item.id === current.id);
        if (!latest?.connection || !belongs(latest, scope)) return;
        delete latest.syncPending;
        await db.putNodes([latest], {}, { completeCreations: [latest.id] });
        const displayed = app.node(current.id);
        if (displayed) {
            displayed.connection = latest.connection;
            delete displayed.syncPending;
        }
        app.notify();
    });
    if (current.kind === 'folder')
        for (const child of app.state.nodes.filter(
            (item) => item.parentId === current.id && !item.connection && !item.syncPending,
        ))
            await created(child.id);
    void driverFor(scope)?.upload();
}

/** Resumes durable jobs. Returns the active run, including a queued explicit retry. Hidden tabs make no requests. */
export function resumeCreations(retry = false): Promise<void> | undefined {
    if (retry) retryRequested = true;
    if (running) return running;
    if (!available()) return;
    const explicitRetry = retryRequested;
    retryRequested = false;
    clearTimeout(timer);
    controller = new AbortController();
    const signal = controller.signal;
    let deferred = false;
    running = (async () => {
        let failed = false;
        deferred = await reconcileFolders(signal);
        const jobs = await db.allSides<CreationJob>('sync-create');
        const all = await db.allNodes();
        jobs.sort((a, b) => ancestors(all, a.id).length - ancestors(all, b.id).length);
        for (const job of jobs) {
            if (job.blocked && !explicitRetry) continue;
            signal.throwIfAborted();
            const run = async () => {
                const saved = await db.getSide<CreationJob>('sync-create', job.id);
                if (!saved) return;
                if (explicitRetry && saved.blocked) {
                    delete saved.blocked;
                    await db.putSide('sync-create', job.id, saved);
                }
                try {
                    await enroll(saved, signal);
                } catch (error) {
                    if ((error as Error).name === 'AbortError') throw error;
                    if (!(await db.getSide('sync-create', job.id))) return;
                    failed = true;
                    failures++;
                    if (error instanceof SyncError && [400, 403, 409, 410, 413, 507].includes(error.status))
                        saved.blocked = true;
                    await db.putSide('sync-create', job.id, saved);
                    const node = (await db.allNodes()).find((item) => item.id === job.id);
                    if (node?.syncPending) {
                        node.syncPending.phase = 'attention';
                        node.syncPending.error = safeError(error).code;
                        await db.putNodes([node]);
                        if (app.node(node.id)?.syncPending)
                            Object.assign(app.node(node.id)!.syncPending!, node.syncPending);
                        app.notify();
                    }
                }
            };
            if (navigator.locks)
                await navigator.locks.request(`mars:enrollment:${job.collection}:${job.member}`, { signal }, run);
            else await run();
        }
        if (!failed) failures = 0;
    })()
        .catch(() => {})
        .finally(async () => {
            running = undefined;
            if (available() && retryRequested) {
                await resumeCreations();
                return;
            }
            if (available())
                void db
                    .allSides<CreationJob>('sync-create')
                    .then((jobs) => {
                        if (deferred || jobs.some((job) => !job.blocked))
                            timer = setTimeout(
                                () => resumeCreations(),
                                Math.min(MAX_RETRY_MS, RETRY_MS * 2 ** Math.min(failures, 5)),
                            );
                    })
                    .catch(() => {});
        });
    return running;
}
const wake = () => {
    if (available()) resumeCreations();
    else {
        controller?.abort();
        clearTimeout(timer);
    }
};
document.addEventListener('visibilitychange', wake);
window.addEventListener('online', wake);
window.addEventListener('offline', wake);
window.addEventListener('pageshow', () => {
    active = true;
    wake();
});
window.addEventListener('pagehide', () => {
    active = false;
    controller?.abort();
    clearTimeout(timer);
});
