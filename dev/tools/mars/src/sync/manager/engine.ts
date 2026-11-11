import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { descendants, newId, textBytes, type TreeNode } from '../../tree/model.ts';
import { openThreads } from '../../share/merge.ts';
import { keyBytes, unwrapKey, verify, verifyGrant, openJson, openCheckpoint } from '../crypto.ts';
import {
    encode,
    decode,
    RejectedPacket,
    validId,
    validGrant,
    validCheckpoint,
    checkpointKey,
    type SignedGrant,
    type SyncBinding,
    type SyncDirectory,
} from '../protocol.ts';
import { Session, stagedSession, type SessionRecord, type ManifestNode } from '../session.ts';
import { materializable, materializationVersion } from '../materialization.ts';
import { Transport } from '../transport.ts';
import { BodyReplica } from '../replica.ts';
import { linkWrappingKey, validLinkWrappingKey, type LiveLink } from '../live-link.ts';
import {
    repository,
    sessions,
    type Scope,
    driverFor,
    drivers,
    scopeId,
    phase,
    belongs,
    rotationRepository,
    preparing,
    scoped,
    materializedViews,
    scopeKey,
    scopes,
    rotations,
    rotationTasks,
    materializations,
    rootFor,
    creations,
    renewing,
    saveScope,
} from './state.ts';
import { continueCleanup } from './live-links.ts';

let initialized = false;
export async function createSession(node: TreeNode, record: SessionRecord) {
    record.binding.participant = record.member.grant.participant;
    record.binding.ownerParticipant = record.owner.grant.participant;
    node.connection = { ...record.binding, phase: 'waiting' };
    if (node.handle) node.manualFileSave = true;
    await db.commitReplica(node.id, null, record, [node], node.kind === 'doc' ? { [node.id]: record.projection } : {});
    const session = new Session(record, repository(node.id));
    sessions.set(node.id, session);
    return session;
}
async function persistProjection(id: string, text: string) {
    if (id !== app.state.currentId) return;
    if (text === viewer.currentText()) {
        sessions.get(id)?.present(text);
        return;
    }
    if (
        viewer.applyConnectionProjection({
            text,
            name: app.node(id)!.name,
            readingWidth: app.node(id)!.localReadingWidth ?? app.node(id)!.readingWidth,
        })
    )
        sessions.get(id)?.present(text);
}
export function start(scope: Scope) {
    if (scope.live && !validLinkWrappingKey(scope.live)) {
        driverFor(scope)?.dispose();
        drivers.delete(scopeId(scope));
        phase(scope, 'attention');
        return;
    }
    if (drivers.has(scopeId(scope))) return;
    const list = () =>
        [...sessions.entries()].filter(
            ([, session]) =>
                session.record.binding.collection === scope.collection &&
                session.record.binding.member === scope.member.grant.member &&
                !session.record.ended,
        );
    const adapter = {
        binding: {
            collection: scope.collection,
            document: scope.root,
            member: scope.member.grant.member,
            permission: scope.member.grant.permission,
            epoch: 1,
            kind: 'sync' as const,
        },
        async prepare() {
            const current = app.node(app.state.currentId);
            if (
                (current && belongs(current, scope) && !viewer.canPublishConnection()) ||
                (await rotationRepository(scope).get())
            )
                return null;
            for (const [, session] of list()) {
                const envelope = await session.prepare();
                if (envelope) {
                    preparing.set(envelope.checkpoint.operation, session);
                    return envelope;
                }
            }
            return null;
        },
        async pending() {
            let count = 0;
            for (const [, session] of list()) count += await session.pending();
            return count;
        },
        async acknowledge(operation: string, digest: string) {
            const session = preparing.get(operation);
            if (!session) throw new Error('Unknown acknowledgement');
            await session.acknowledge(operation, digest);
            preparing.delete(operation);
        },
        async receive() {
            return null;
        },
    };
    const driver = new Transport({
        origin: location.origin,
        token: scope.token,
        ownerPublicKey: scope.owner.grant.publicKey,
        replica: adapter,
        exclude: () => scope.exclusions,
        streams: ['body', 'review', 'tree'],
        collectionScope: scope.kind === 'folder' || (scope.selection?.length ?? 0) > 1,
        authorValid: (author) => verifyGrant(author, scope.owner.grant),
        directory: scope.kind === 'folder',
        wanted: (key) => {
            const checkpoint = checkpointKey(key);
            return !checkpoint || list().some(([, session]) => session.record.binding.document === checkpoint.document);
        },
        known: async (key) => {
            const checkpoint = checkpointKey(key);
            if (!checkpoint || checkpoint.collection !== scope.collection) return false;
            const entry = list().find(([, session]) => session.record.binding.document === checkpoint.document);
            if (!entry) return false;
            const applied = (record?: SessionRecord) =>
                !!record &&
                !record.ended &&
                record.binding.collection === scope.collection &&
                record.binding.member === scope.member.grant.member &&
                record.binding.document === checkpoint.document &&
                (record.progress[`${checkpoint.member}/${checkpoint.stream}`] ?? 0) >= checkpoint.sequence;
            // Session.record is a committed snapshot. Only consult storage when
            // another tab might have received a packet this tab has not seen.
            return applied(entry[1].record) || applied(await entry[1].repository.get());
        },
        status: (state) => phase(scope, state),
        projection: async () => {},
        keyChanged: async (envelope, signal) => {
            const session = list().find(
                ([, item]) => item.record.binding.document === envelope.checkpoint.document,
            )?.[1];
            if (!session) return;
            const { epoch } = await scoped(scope, 'epoch', { document: envelope.checkpoint.document }, signal);
            await refreshKey(scope, session, epoch, signal);
            preparing.delete(envelope.checkpoint.operation);
        },
        receive: async (envelope, author, signal, directory) => {
            const entry = list().find(
                ([, session]) => session.record.binding.document === envelope.checkpoint.document,
            );
            if (!entry) return null;
            const [id, session] = entry;
            if (!app.node(id)?.connection || scope.exclusions.includes(envelope.checkpoint.document)) return null;
            if (envelope.checkpoint.epoch > session.record.binding.epoch)
                await refreshKey(scope, session, envelope.checkpoint.epoch, signal);
            const draft = id === app.state.currentId ? viewer.connectionDraft() : null;
            if (draft && envelope.checkpoint.stream === 'body') {
                const staged = new BodyReplica(session.presented.checkpoint());
                const anchor = staged.anchor(draft.start, draft.end);
                try {
                    const key = session.record.keys?.[String(envelope.checkpoint.epoch)] ?? session.record.key;
                    try {
                        staged.apply(await openCheckpoint(envelope, decode(key), author.grant));
                    } catch (error) {
                        throw new RejectedPacket(error);
                    }
                    if (!staged.unchanged(anchor))
                        viewer.setConnectionDraftConflict(() => {
                            void recoverDraft(id, session, envelope, author);
                        });
                } finally {
                    staged.destroy();
                }
                return null;
            }
            if (id === app.state.currentId && !viewer.canApplyConnection()) return null;
            if (id === app.state.currentId) await viewer.flush();
            const text = await session.receive(envelope, author);
            const meta = session.metadata(),
                node = app.node(id)!;
            if (typeof meta.name === 'string') node.name = meta.name;
            if (meta.readingWidth === 'full' || typeof meta.readingWidth === 'number')
                node.readingWidth = meta.readingWidth;
            // Any editor can write `meta.kind` in the shared replica. Only the root of a folder scope
            // carries a manifest, so decide from the local binding, not from received content.
            if (scope.kind === 'folder' && session.record.binding.document === scope.root && session.folder())
                await materialize(scope, session, signal, { directory });
            await persistProjection(id, text);
            return text;
        },
        cycle: async (signal, directory) => {
            if (scope.kind === 'folder') {
                directory ??= await scoped(scope, 'directory', {}, signal);
                const root = sessions.get(scope.localRoot);
                // A previous download can fail after its manifest was saved.
                // Recover from that durable manifest even when no packet needs fetching.
                if (root) {
                    await root.synchronize();
                    if (
                        materializedViews.get(scopeId(scope)) !==
                        materializationVersion(scope.localRoot, root.record.body, scope.exclusions, directory!)
                    )
                        await materialize(scope, root, signal, { directory });
                }
                await reconcileDirectory(scope, signal, directory);
            }
            const current = sessions.get(app.state.currentId ?? '');
            if (current?.record.binding.collection === scope.collection && viewer.canApplyConnection())
                await persistProjection(app.state.currentId!, current.record.projection);
        },
    });
    drivers.set(scopeId(scope), driver);
    driver.start();
}
export async function initialize(force = false) {
    if (initialized && !force) return;
    initialized = true;
    if (await db.repairDuplicateConnections()) await app.load();
    const records = await db.allSides<SessionRecord>('sync');
    for (const record of records) {
        if (record?.kind !== 'connected') continue;
        const node = app.state.nodes.find(
            (n) =>
                n.connection?.document === record.binding.document &&
                n.connection.collection === record.binding.collection &&
                n.connection.member === record.binding.member,
        );
        if (!node) continue;
        const scope =
            (await db.getMeta<Scope>(scopeKey(record.binding.collection, record.binding.member))) ??
            (await db.getMeta<Scope>(`sync:scope:${record.binding.collection}`));
        if (scope?.member.grant.member !== record.binding.member) continue;
        if (!scope) continue;
        const active = scopes.get(scopeId(scope));
        if (active) Object.assign(active, scope);
        else scopes.set(scopeId(scope), scope);
        if (!sessions.has(node.id)) sessions.set(node.id, new Session(record, repository(node.id)));
    }
    for (const [id, session] of sessions)
        if (!app.node(id)?.connection) {
            session.destroy();
            sessions.delete(id);
        }
    for (const scope of scopes.values()) {
        const wasRotating = rotations.has(scopeId(scope));
        const rotation = await rotationRepository(scope).get();
        if (rotation) {
            rotations.set(scopeId(scope), rotation);
            if (!rotationTasks.has(scopeId(scope))) driverFor(scope)?.stop();
            phase(scope, 'attention');
        } else rotations.delete(scopeId(scope));
        if (scope.cleanup) {
            void continueCleanup(scope).catch(() => phase(scope, 'attention'));
            continue;
        }
        if (
            !rotation &&
            [...sessions.values()].some(
                (s) =>
                    s.record.binding.collection === scope.collection &&
                    s.record.binding.member === scope.member.grant.member &&
                    !s.record.ended,
            )
        )
            start(scope);
        if (!rotation && wasRotating) driverFor(scope)?.retry();
        if (!document.hidden && scope.kind === 'folder' && app.node(scope.localRoot)?.open)
            void opened(scope.localRoot).catch(() => {});
    }
}
export async function refresh(ids: string[]) {
    await initialize(true);
    for (const [id, session] of sessions) {
        const node = app.node(id);
        if (!node?.connection) {
            session.destroy();
            sessions.delete(id);
            continue;
        }
        if (!ids.includes(id)) continue;
        const text = await session.synchronize();
        if (id === app.state.currentId && viewer.canApplyConnection()) await persistProjection(id, text);
    }
    for (const scope of scopes.values()) {
        const stored = await db.getMeta<Scope>(scopeId(scope));
        if (stored) Object.assign(scope, stored);
        if (!app.state.nodes.some((node) => belongs(node, scope))) {
            driverFor(scope)?.dispose();
            drivers.delete(scopeId(scope));
        }
    }
    viewer.refreshConnection();
}
export async function present(id: string) {
    await initialize();
    const session = sessions.get(id);
    if (!session) return;
    await persistProjection(id, await session.synchronize());
}
export async function snapshot(node: TreeNode, text: string) {
    await initialize();
    const session = sessions.get(node.id);
    if (!session) throw new Error('This replica is not ready');
    await validateSnapshot(session, text);
    await persist(node, text);
}
export async function validateSnapshot(session: Session, text: string) {
    let record = structuredClone(await session.repository.get());
    if (!record) throw new Error('This replica is unavailable');
    const staged = new Session(record, {
        get: async () => structuredClone(record),
        commit: async (_expected, next) => {
            record = structuredClone(next);
        },
    });
    try {
        await staged.change(record.projection, text);
    } finally {
        staged.destroy();
    }
}
export async function loadPackets(scope: Scope, session: Session, signal?: AbortSignal, staged = false) {
    const info = await scoped(scope, 'document-info', { document: session.record.binding.document }, signal);
    const stored = await session.repository.get();
    if (stored) {
        stored.creator = info.creator;
        await session.repository.commit(stored.revision, { ...stored, revision: stored.revision + 1 });
        session.record = { ...stored, revision: stored.revision + 1 };
    }
    const keys: string[] = [];
    let after: string | undefined;
    do {
        const page = await scoped(scope, 'index', { document: session.record.binding.document, after }, signal);
        keys.push(...page.entries.map((e: { key: string }) => e.key));
        if (page.complete) break;
        after = page.cursor;
    } while (after);
    const priority = (key: string) =>
        key.includes(`/${scope.owner.grant.member}/body/`) || key.includes(`/${scope.owner.grant.member}/tree/`)
            ? 0
            : key.includes(`/${info.creator}/review/`)
              ? 1
              : 2;
    keys.sort((a, b) => priority(a) - priority(b));
    for (const key of keys) {
        const packet = await scoped(scope, 'get', { key }, signal);
        // The service response is untrusted input. A malformed or unverifiable checkpoint is skipped,
        // so one bad upload from another participant cannot block a join.
        if (!validCheckpoint(packet.envelope) || !validGrant(packet.author)) continue;
        if (packet.envelope.checkpoint.epoch > session.record.binding.epoch)
            await refreshKey(scope, session, packet.envelope.checkpoint.epoch, signal, staged);
        try {
            await session.receive(packet.envelope, packet.author);
        } catch (error) {
            if (!(error instanceof RejectedPacket)) throw error;
        }
    }
    session.present(session.record.projection);
}
export async function materialize(
    scope: Scope,
    rootSession: Session,
    signal?: AbortSignal,
    options: {
        bootstrap?: TreeNode;
        progress?: (completed: number, total: number) => void;
        directory?: SyncDirectory;
    } = {},
) {
    const { bootstrap, progress } = options;
    const previous = materializations.get(scopeId(scope)) ?? Promise.resolve();
    const task = previous
        .catch(() => {})
        .then(async () => {
            signal?.throwIfAborted();
            await rootSession.synchronize();
            const body = rootSession.record.body,
                manifest = rootSession.manifest();
            const directory = options.directory ?? (await scoped(scope, 'directory', {}, signal));
            const version = materializationVersion(scope.localRoot, body, scope.exclusions, directory);
            const registered = new Map<string, { kind: string; deleted: boolean }>(
                directory.documents.map((item: { id: string; kind: string; deleted: boolean }) => [item.id, item]),
            );
            const values = materializable(scope.root, manifest, registered, scope.exclusions);
            const current = (await db.allNodes()).filter((node) => belongs(node, scope));
            const mapping = new Map(current.map((node) => [node.connection!.document, node]));
            if (
                !bootstrap &&
                mapping.size === current.length &&
                mapping.get(scope.root)?.id === scope.localRoot &&
                values.every((item) => {
                    const node = mapping.get(item.id),
                        parent = mapping.get(item.parent!);
                    return (
                        node?.kind === item.kind &&
                        parent?.kind === 'folder' &&
                        node.parentId === parent.id &&
                        node.name === item.name &&
                        node.order === (Number.isSafeInteger(item.order) ? item.order : 0)
                    );
                })
            ) {
                materializedViews.set(scopeId(scope), version);
                return;
            }
            const entries: { node: TreeNode; parent: string; record?: SessionRecord }[] = [];
            const total = values.filter((item) => item.kind === 'doc' && !mapping.has(item.id)).length;
            let completed = 0;
            progress?.(completed, total);
            for (const item of values) {
                const existing = mapping.get(item.id),
                    local = existing?.id ?? newId();
                const node: TreeNode = {
                    id: local,
                    parentId: null,
                    kind: item.kind,
                    name: item.name,
                    order: Number.isSafeInteger(item.order) ? item.order : 0,
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                    originId: item.origin ?? local,
                    connection: {
                        collection: scope.collection,
                        document: item.id,
                        member: scope.member.grant.member,
                        permission: scope.member.grant.permission,
                        kind: 'sync',
                        epoch: item.epoch ?? 1,
                        root: scope.root,
                        phase: 'working',
                    },
                };
                const entry = { node, parent: item.parent! } as (typeof entries)[number];
                entries.push(entry);
                if (!existing && item.kind === 'doc') {
                    const record = Session.seed(
                        node.connection!,
                        '',
                        scope.identity,
                        scope.token,
                        item.key!,
                        scope.owner,
                        item.name,
                        scope.member,
                    );
                    const empty = new BodyReplica();
                    record.body = encode(empty.checkpoint());
                    record.binding.participant = scope.member.grant.participant;
                    record.binding.ownerParticipant = scope.owner.grant.participant;
                    empty.destroy();
                    const staged = stagedSession(record);
                    try {
                        await loadPackets(scope, staged, signal, true);
                        entry.record = staged.record;
                        const meta = staged.metadata();
                        Object.assign(node, {
                            size: textBytes(staged.record.projection),
                            threads: openThreads(staged.record.projection),
                            ...(validId(meta.origin) ? { originId: meta.origin } : {}),
                            ...(meta.readingWidth === 'full' || typeof meta.readingWidth === 'number'
                                ? { readingWidth: meta.readingWidth }
                                : {}),
                        });
                        node.connection = { ...staged.record.binding, phase: 'working' };
                    } finally {
                        staged.destroy();
                    }
                    progress?.(++completed, total);
                }
            }
            signal?.throwIfAborted();
            await app.serializePersistence(async () => {
                signal?.throwIfAborted();
                const root = bootstrap ?? app.node(scope.localRoot);
                if (!root?.connection) throw new db.ReplicaConflict('missing-node');
                await db.materializeConnected(
                    root,
                    body,
                    entries,
                    bootstrap ? { record: rootSession.record, metadata: { [scopeId(scope)]: scope } } : undefined,
                );
                await app.load();
            });
            materializedViews.set(scopeId(scope), version);
            // Only committed replicas become visible to upload/poll drivers.
            if (bootstrap) scopes.set(scopeId(scope), scope);
            await initialize(true);
        });
    materializations.set(scopeId(scope), task);
    try {
        await task;
    } finally {
        if (materializations.get(scopeId(scope)) === task) materializations.delete(scopeId(scope));
    }
}
async function reconcileDirectory(scope: Scope, signal?: AbortSignal, received?: SyncDirectory) {
    const directory = received ?? (await scoped(scope, 'directory', {}, signal));
    for (const doc of directory.documents)
        if (doc.deleted) {
            const node = app.state.nodes.find((n) => belongs(n, scope) && n.connection!.document === doc.id);
            if (node) node.connection!.phase = 'ended';
        }
}
export async function persist(node: TreeNode, text: string) {
    await initialize();
    const session = sessions.get(node.id);
    if (!session) throw new Error('This replica is not ready');
    const next = await session.change(session.record.projection, text);
    await persistProjection(node.id, next);
    setTimeout(() => {
        void driverFor(rootFor(node))?.upload();
    }, 0);
    const scope = rootFor(node);
    if (scope && (!scope.lastRenewed || Date.now() - scope.lastRenewed > 86400000)) void opened(node.id);
}
export async function metadata(id: string) {
    await initialize();
    const node = app.node(id);
    if (!node?.connection) return;
    const session = sessions.get(id);
    if (session)
        await session.metadataChanged({
            name: node.name,
            ...(node.readingWidth !== undefined ? { readingWidth: node.readingWidth } : {}),
        });
    const scope = rootFor(node);
    if (scope?.kind === 'folder') await updateManifest(scope);
    void driverFor(rootFor(node))?.upload();
}
async function updateManifest(scope: Scope) {
    const root = sessions.get(scope.localRoot);
    if (!root) return;
    const previous = new Map(root.manifest().map((n) => [n.id, n]));
    const values = app.state.nodes
        .filter((n) => belongs(n, scope))
        .map((n): ManifestNode => ({
            ...(previous.get(n.connection!.document) ?? {}),
            id: n.connection!.document,
            kind: n.kind,
            name: n.name,
            order: n.order,
            parent: n.id === scope.localRoot ? null : (app.node(n.parentId)?.connection?.document ?? scope.root),
        }));
    await root.editManifest(values);
}
export async function created(id: string) {
    const active = creations.get(id);
    if (active) return active;
    const task = enrollCreated(id);
    creations.set(id, task);
    try {
        await task;
    } finally {
        if (creations.get(id) === task) creations.delete(id);
    }
}
async function enrollCreated(id: string) {
    await initialize();
    const node = app.node(id),
        parent = app.node(node?.parentId ?? null),
        scope = parent ? rootFor(parent) : undefined;
    if (!node || node.connection || !scope || scope.kind !== 'folder') return;
    const remote = newId(),
        key = encode(keyBytes());
    await scoped(scope, 'document', { document: remote, kind: node.kind });
    const binding: SyncBinding = {
        collection: scope.collection,
        document: remote,
        member: scope.member.grant.member,
        permission: scope.member.grant.permission,
        kind: 'sync',
        epoch: 1,
        root: scope.root,
    };
    await app.serializePersistence(async () => {
        if (
            !app.node(id) ||
            app.node(id)?.connection ||
            app.node(node.parentId)?.connection?.collection !== scope.collection
        )
            return;
        if (node.kind === 'doc') {
            const record = Session.seed(
                binding,
                await app.docText(id),
                scope.identity,
                scope.token,
                key,
                scope.owner,
                node.name,
                scope.member,
            );
            const session = await createSession(node, record);
            await session.initial();
            await session.metadataChanged({ origin: validId(node.originId) ? node.originId : node.id });
        } else {
            node.connection = { ...binding, phase: 'waiting' };
            await db.putNodes([node]);
        }
    });
    if (!node.connection) return;
    const root = sessions.get(scope.localRoot)!;
    await root.editManifest([
        {
            id: remote,
            kind: node.kind,
            parent: parent!.connection!.document,
            name: node.name,
            order: node.order,
            ...(node.kind === 'doc' ? { key, origin: node.id } : {}),
        },
    ]);
    if (node.kind === 'folder')
        for (const child of app.state.nodes.filter((item) => item.parentId === node.id)) await created(child.id);
    void driverFor(scope)?.upload();
}
export async function opened(id: string) {
    await initialize();
    const node = app.node(id);
    if (!node?.connection || node.connection.phase === 'ended' || document.hidden) return;
    const scope = rootFor(node);
    if (!scope || (scope.live && !validLinkWrappingKey(scope.live))) return;
    start(scope);
    if (scope.lastRenewed && Date.now() - scope.lastRenewed < 86400000) return;
    if (renewing.has(scopeId(scope))) return renewing.get(scopeId(scope));
    const root = app.node(scope.localRoot)!;
    const targets =
        scope.kind === 'folder'
            ? [root, ...descendants(app.state.nodes, root.id)].filter((item) => belongs(item, scope))
            : [node];
    const controller = new AbortController(),
        pause = () => {
            if (document.hidden) controller.abort();
        },
        leave = () => controller.abort();
    document.addEventListener('visibilitychange', pause);
    window.addEventListener('pagehide', leave);
    const run = (async () => {
        let complete = true;
        for (const target of targets) {
            if (controller.signal.aborted) {
                complete = false;
                break;
            }
            const session = sessions.get(target.id);
            try {
                if (!session) {
                    if (target.kind === 'folder')
                        await scoped(
                            scope,
                            'touch-folder',
                            { document: target.connection!.document },
                            controller.signal,
                        );
                    continue;
                }
                const entries: { key: string }[] = [];
                let after: string | undefined;
                do {
                    const page = await scoped(
                        scope,
                        'index',
                        { document: target.connection!.document, after },
                        controller.signal,
                    );
                    entries.push(...page.entries);
                    if (page.complete) break;
                    after = page.cursor;
                } while (after);
                const seen = new Set<string>();
                for (const entry of entries) {
                    const parts = entry.key.split('/'),
                        group = parts.slice(0, -1).join('/');
                    if (seen.has(group)) continue;
                    seen.add(group);
                    await scoped(
                        scope,
                        'touch-record',
                        { document: target.connection!.document, key: entry.key },
                        controller.signal,
                    );
                }
                await scoped(scope, 'touch', { document: target.connection!.document }, controller.signal);
            } catch (error) {
                complete = false;
                if (!controller.signal.aborted)
                    viewer.toast(
                        'Could not extend Sync activity. Your local copies are retained. Try again when the connection is available.',
                    );
            }
        }
        if (complete) {
            scope.lastRenewed = Date.now();
            await saveScope(scope);
        }
    })();
    renewing.set(scopeId(scope), run);
    try {
        await run;
    } finally {
        document.removeEventListener('visibilitychange', pause);
        window.removeEventListener('pagehide', leave);
        renewing.delete(scopeId(scope));
    }
}
const renewVisibleFolders = () => {
    if (!document.hidden)
        for (const scope of scopes.values())
            if (scope.kind === 'folder' && app.node(scope.localRoot)?.open)
                void opened(scope.localRoot).catch(() => {});
};
document.addEventListener('visibilitychange', renewVisibleFolders);
window.addEventListener('pageshow', renewVisibleFolders);
export async function undo(id: string, redo = false) {
    const session = sessions.get(id);
    if (!session) return;
    const text = await session.undo(redo);
    await persistProjection(id, text);
    void driverFor(rootFor(app.node(id)!))?.upload();
}
export async function markReviewed(id: string) {
    const session = sessions.get(id);
    if (!session) return;
    await session.markReviewed();
    void driverFor(rootFor(app.node(id)!))?.upload();
}
export async function undoReview(
    id: string,
    before: import('../../share/merge.ts').Thread[],
    after: import('../../share/merge.ts').Thread[],
) {
    const session = sessions.get(id);
    if (!session) return;
    const text = await session.undoReview(before, after);
    await persistProjection(id, text);
    void driverFor(rootFor(app.node(id)!))?.upload();
}
export async function sharedWidth(id: string, width: number | 'full') {
    const session = sessions.get(id);
    if (!session) return;
    await session.metadataChanged({ readingWidth: width });
    const node = app.node(id)!;
    node.readingWidth = width;
    void driverFor(rootFor(node))?.upload();
}
export async function readLiveKey(
    update: { value: { document: string; epoch: number; nonce: string; ciphertext: string }; signature: string },
    link: LiveLink,
    owner: SignedGrant,
) {
    if (!(await verify(update.value, update.signature, owner.grant.publicKey)))
        throw new Error('Invalid key update signature');
    return openJson<{ epoch: number; key: string }>(update.value, linkWrappingKey(link), { invitation: link.id });
}
async function refreshKey(scope: Scope, session: Session, epoch: number, signal?: AbortSignal, staged = false) {
    const id = session.record.binding.document,
        response = await scoped(scope, 'key-read', { document: id }, signal);
    let key: string | undefined;
    for (const value of response.values) {
        if (
            value.wrapped.epoch === epoch &&
            value.wrapped.document === id &&
            value.wrapped.member === scope.member.grant.member &&
            (await verify(value.wrapped, value.signature, scope.owner.grant.publicKey))
        )
            key = encode(await unwrapKey(value.wrapped, scope.identity));
    }
    if (!key && scope.live) {
        const bootstrap = await scoped(scope, 'bootstrap', {}, signal);
        if (bootstrap.keyUpdate) {
            const updated = await readLiveKey(bootstrap.keyUpdate, scope.live, scope.owner);
            if (updated.epoch === epoch) key = updated.key;
        }
    }
    if (!key) throw new Error('A new document key is required. Local work is retained.');
    await session.rekey(epoch, key);
    if (staged) {
        if (id === scope.root) scope.key = key;
        return;
    }
    const node = app.state.nodes.find((n) => belongs(n, scope) && n.connection!.document === id);
    if (node) node.connection!.epoch = epoch;
    if (id === scope.root) {
        scope.key = key;
        await saveScope(scope);
    }
}
async function recoverDraft(
    id: string,
    session: Session,
    envelope: import('../protocol.ts').SignedCheckpoint,
    author: SignedGrant,
) {
    const draft = viewer.connectionDraft();
    if (!draft) return;
    const original = app.node(id);
    if (!original) return;
    const copy = await app.createDoc({
        name: `${original.name} (recovered draft)`,
        text: draft.text,
        parentId: null,
        open: false,
    });
    viewer.discardConnectionDraft();
    const text = await session.receive(envelope, author);
    await persistProjection(id, text);
    await app.openDoc(copy.id);
    viewer.toast('Draft saved as an independent local copy');
}
