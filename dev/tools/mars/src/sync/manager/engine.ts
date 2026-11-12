// Replication engine of the connection manager. It loads saved connections, runs one transport per
// scope, applies received checkpoints, builds folder hierarchies, and renews activity leases.
// It also carries local edits, metadata, undo, and review actions into the session of a document.
// A session verifies and decrypts a checkpoint. This module decides when the viewer shows the result.
// Service responses are untrusted. Grants, key updates, and checkpoints must verify against the owner
// key of the scope before use.
// A folder hierarchy becomes visible in one storage transaction, after every new document downloads.
// The module registers `visibilitychange` and `pageshow` listeners for lease renewal when it loads.
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
import { synchronizeDeletions, resumeDeletions, personalSync } from './deletion.ts';

// True after the first `initialize` call starts. The call sets it before its first await.
let initialized = false;
/**
 * Binds a local node to a seeded replica record and registers its session.
 * One transaction saves the record, the node, and the projection of a document.
 * Throws `ReplicaConflict` when the node already has a replica record.
 * The call changes `node` and `record.binding` before the write. A linked file loses automatic saves.
 */
export async function createSession(node: TreeNode, record: SessionRecord) {
    record.binding.participant = record.member.grant.participant;
    record.binding.ownerParticipant = record.owner.grant.participant;
    node.connection = { ...record.binding, phase: 'waiting' };
    // Remote edits never write to a linked file. A connected document needs an explicit disk Save.
    if (node.handle) node.manualFileSave = true;
    await db.commitReplica(node.id, null, record, [node], node.kind === 'doc' ? { [node.id]: record.projection } : {});
    const session = new Session(record, repository(node.id));
    sessions.set(node.id, session);
    return session;
}
/**
 * Shows a committed projection in the viewer when `id` is the open document.
 * The session's presented state advances only when the viewer shows that text. The next local
 * change is therefore a diff against what the user sees.
 * The viewer refuses the text while an editor is open or unsaved input exists. A later call applies it.
 */
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
/**
 * Creates and starts the transport of a scope. A scope with a transport stays unchanged.
 * The transport uploads the outboxes of every active session in the scope and polls for checkpoints.
 * A Live scope whose link has no independent wrapping key gets no transport and shows `attention`.
 */
export function start(scope: Scope) {
    if (scope.live && !validLinkWrappingKey(scope.live)) {
        driverFor(scope)?.dispose();
        drivers.delete(scopeId(scope));
        phase(scope, 'attention');
        return;
    }
    if (drivers.has(scopeId(scope))) return;
    // The active sessions of this scope. The registry can change between polls, so read it each time.
    const list = () =>
        [...sessions.entries()].filter(
            ([, session]) =>
                session.record.binding.collection === scope.collection &&
                session.record.binding.member === scope.member.grant.member &&
                !session.record.ended,
        );
    // One adapter presents all sessions of the scope to the transport as a single replica.
    const adapter = {
        // The transport reads only the collection, member, and document of this binding.
        binding: {
            collection: scope.collection,
            document: scope.root,
            member: scope.member.grant.member,
            permission: scope.member.grant.permission,
            epoch: 1,
            kind: 'sync' as const,
        },
        async prepare() {
            // Uploads wait while the open document of this scope has input that the viewer has not
            // saved, and while a key rotation is unfinished.
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
        // Unused: the `receive` option below takes every downloaded checkpoint.
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
        // Accepts a grant that the owner signed, or a Live grant under an invitation that the owner signed.
        authorValid: (author) => verifyGrant(author, scope.owner.grant),
        directory: scope.kind === 'folder',
        beforeUpload: personalSync(scope)
            ? async (signal) => {
                  await synchronizeDeletions(scope, signal);
              }
            : undefined,
        deleted: personalSync(scope) ? (signal) => synchronizeDeletions(scope, signal) : undefined,
        // A document without a local session waits for its folder manifest. Its ciphertext stays on the service.
        wanted: (key) => {
            const checkpoint = checkpointKey(key);
            return !checkpoint || list().some(([, session]) => session.record.binding.document === checkpoint.document);
        },
        // True when saved progress already covers the checkpoint, so the transport skips the download.
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
        // `receive` and `cycle` update the viewer themselves.
        projection: async () => {},
        // The service refused an upload because the document has a newer key epoch. Install the key
        // of that epoch. A rekey gives each outbox packet a new operation ID, so the old entry is stale.
        keyChanged: async (envelope, signal) => {
            const session = list().find(
                ([, item]) => item.record.binding.document === envelope.checkpoint.document,
            )?.[1];
            if (!session) return;
            const { epoch } = await scoped(scope, 'epoch', { document: envelope.checkpoint.document }, signal);
            await refreshKey(scope, session, epoch, signal);
            preparing.delete(envelope.checkpoint.operation);
        },
        // Returns the new projection, or `null` to leave the checkpoint for a later poll.
        // The transport has already verified the author grant.
        receive: async (envelope, author, signal, directory) => {
            const entry = list().find(
                ([, session]) => session.record.binding.document === envelope.checkpoint.document,
            );
            if (!entry) return null;
            const [id, session] = entry;
            if (!app.node(id)?.connection || scope.exclusions.includes(envelope.checkpoint.document)) return null;
            if (envelope.checkpoint.epoch > session.record.binding.epoch)
                await refreshKey(scope, session, envelope.checkpoint.epoch, signal);
            // An open block or table-cell editor holds a draft that the replica does not contain yet.
            // Apply the checkpoint to a copy only, to learn whether it changes the edited range.
            // A change there shows the recovery banner. In both cases the checkpoint waits for a later poll.
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
            // The viewer cannot take a new projection now. Leave the checkpoint for a later poll.
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
        // Runs after each complete poll, with the directory of the first index page when one arrived.
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
            // Show a committed projection that the viewer refused earlier.
            const current = sessions.get(app.state.currentId ?? '');
            if (current?.record.binding.collection === scope.collection && viewer.canApplyConnection())
                await persistProjection(app.state.currentId!, current.record.projection);
        },
    });
    drivers.set(scopeId(scope), driver);
    driver.start();
}
/**
 * Loads the saved connections of this profile into the registries and starts their transports.
 * The first call does the work. Later calls return at once unless `force` is true.
 * A forced call reloads scopes from storage, keeps existing sessions, and drops the sessions of
 * nodes that lost their binding.
 * Each run shows `attention` for a scope with a saved key-rotation plan, and resumes Live-only cleanup.
 * A call that arrives while the first load is in progress returns before sessions exist.
 * Side effects: a duplicate-connection repair in storage, network polls, and lease renewal.
 */
export async function initialize(force = false) {
    if (initialized && !force) return;
    initialized = true;
    resumeDeletions();
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
        // A scope can also sit under a key without the member ID. Use it only for its own member.
        const scope =
            (await db.getMeta<Scope>(scopeKey(record.binding.collection, record.binding.member))) ??
            (await db.getMeta<Scope>(`sync:scope:${record.binding.collection}`));
        if (scope?.member.grant.member !== record.binding.member) continue;
        if (!scope) continue;
        // Other modules hold references to a loaded scope, so update it in place.
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
        // A saved rotation plan pauses publication. A rotation run in this tab manages the transport
        // itself. Without such a run, the transport stops until the plan is gone.
        if (rotation) {
            rotations.set(scopeId(scope), rotation);
            if (!rotationTasks.has(scopeId(scope))) driverFor(scope)?.stop();
            phase(scope, 'attention');
        } else rotations.delete(scopeId(scope));
        // Cleanup deletes the remote copy and stops the transport, so do not start one.
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
        // Storage has no plan but this tab still listed one: another tab finished the rotation.
        if (!rotation && wasRotating) driverFor(scope)?.retry();
        if (!document.hidden && scope.kind === 'folder' && app.node(scope.localRoot)?.open)
            void opened(scope.localRoot).catch(() => {});
    }
}
/**
 * Brings this tab in line with storage. The app calls it after another tab changes the library and
 * after the viewer finishes a save of a connected document.
 * Reloads the registries, reads the stored replica of each connected node in `ids` again, and shows
 * the open document when the viewer can take it. A scope without bound nodes loses its transport.
 */
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
/**
 * Shows the stored projection of connected document `id` in the viewer after the document opens.
 * It also repairs an outdated stored projection. An unconnected document is left unchanged.
 */
export async function present(id: string) {
    await initialize();
    const session = sessions.get(id);
    if (!session) return;
    await persistProjection(id, await session.synchronize());
}
/**
 * Replaces the text of a connected document with a complete incoming text, as one local change.
 * Throws when the document has no session, or when the grant of this member forbids the change.
 * The change runs on a private copy first, so a refused change leaves the live session and storage unchanged.
 */
export async function snapshot(node: TreeNode, text: string) {
    await initialize();
    const session = sessions.get(node.id);
    if (!session) throw new Error('This replica is not ready');
    await validateSnapshot(session, text);
    await persist(node, text);
}
/**
 * Applies `text` to a private copy of the stored record, and throws what the real change would throw.
 * It writes nothing to storage. A session keeps staged state from a change that fails, so callers
 * run this check before they change the live session.
 */
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
/**
 * Downloads every stored checkpoint of one document and applies it to `session`. A device uses it
 * for the first fill of a replica that it joins.
 * The service supplies the index, the checkpoints, and the author grants, all untrusted. The session
 * verifies each author grant against the owner key, and each signature and ciphertext, before a commit.
 * A malformed or rejected checkpoint is skipped. Other failures, an abort included, propagate.
 * `staged` marks a private session: a key refresh then leaves tree nodes and saved scopes unchanged.
 */
export async function loadPackets(scope: Scope, session: Session, signal?: AbortSignal, staged = false) {
    // The creator of a document publishes its seed comments. A session accepts a seed from no one else.
    // The service names the creator. That member still needs a grant that verifies against the owner key.
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
    // Order matters. The owner's body and tree checkpoints can carry the keys of earlier epochs, and
    // the creator's review checkpoint carries the seed comments. Later checkpoints can need both.
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
/**
 * Makes the local tree of a folder scope match the manifest in its root replica.
 * It selects the manifest entries with a complete, registered parent chain, downloads each new
 * document into a private staged session, and then publishes all nodes and replicas in one transaction.
 * No incomplete hierarchy becomes visible. Calls for one scope run one after another.
 * Throws on abort, on a failed download, and with `ReplicaConflict` when the stored root changed
 * during the downloads. A later call reads the latest manifest.
 * @param options.bootstrap The unsaved root node of a first pairing. The same transaction then also
 *   saves the root replica and the scope.
 * @param options.progress Receives the number of downloaded documents and the number to download.
 * @param options.directory The registered documents from the service. Fetched when absent.
 */
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
    // Wait for the previous materialization of this scope. Its failure does not block this one.
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
            // The manifest is content that any editor of the folder can write. `materializable`
            // keeps well-formed entries only, and never places an entry with a broken chain at the root.
            const values = materializable(scope.root, manifest, registered, scope.exclusions);
            const current = (await db.allNodes()).filter((node) => belongs(node, scope));
            const mapping = new Map(current.map((node) => [node.connection!.document, node]));
            // The stored tree already matches: one node per remote ID, each with the right kind,
            // parent, name, and order. Record the version and skip the tree write and its broadcast.
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
                // Download a new document into a private session. Storage, the visible tree, and the
                // transport do not see it before the final transaction.
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
                    // Only the creator seeds a replica. A device that joins starts from an empty
                    // replica and takes the creator's checkpoint.
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
                // The persistence queue can make this step wait. Check for a cancellation again.
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
/**
 * Marks the tree nodes of documents that the directory reports as deleted with the `ended` phase.
 * The change is in memory only. It fetches the directory when the poll did not supply one.
 */
async function reconcileDirectory(scope: Scope, signal?: AbortSignal, received?: SyncDirectory) {
    const directory = received ?? (await scoped(scope, 'directory', {}, signal));
    for (const doc of directory.documents)
        if (doc.deleted) {
            const node = app.state.nodes.find((n) => belongs(n, scope) && n.connection!.document === doc.id);
            if (node) node.connection!.phase = 'ended';
        }
}
/**
 * Commits the new text of a connected document as a local change, then requests an upload.
 * The session saves the change and its outbox entry in one transaction before any network use.
 * Throws when the document has no session, when the grant forbids the change, or when the
 * connection has ended. It also starts a lease renewal when the last one is older than 24 hours.
 */
export async function persist(node: TreeNode, text: string) {
    await initialize();
    const session = sessions.get(node.id);
    if (!session) throw new Error('This replica is not ready');
    const next = await session.change(session.record.projection, text);
    await persistProjection(node.id, next);
    // The viewer counts this save as in progress until the caller returns, and `prepare` publishes
    // nothing during a save. Upload in a later task.
    setTimeout(() => {
        void driverFor(rootFor(node))?.upload();
    }, 0);
    const scope = rootFor(node);
    // 86400000 ms is 24 hours, the minimum interval between lease renewals of one scope.
    if (scope && (!scope.lastRenewed || Date.now() - scope.lastRenewed > 86400000)) void opened(node.id);
}
/**
 * Publishes the current name and shared reading width of a connected node after a rename or a move.
 * In a folder scope it also writes the name, order, and parent of every bound node to the manifest.
 * Throws `SyncError` 403 when the grant of this member does not permit the change.
 */
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
/**
 * Writes the local tree of a folder scope to its manifest. Each entry keeps its other fields, such
 * as the document key. A node whose local parent is not in the scope goes under the root.
 */
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
/**
 * Adds a node that was created in, or moved into, a synced folder to that folder's collection.
 * Concurrent calls for one node share one enrollment. The call does nothing when the node is gone,
 * is already connected, or has no parent in a folder scope.
 */
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
/**
 * Registers a remote document for the node, binds the node, and adds it to the folder manifest.
 * A folder also enrolls its children. The registration request precedes every local write.
 */
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
        // The tree can change during the registration request. Bind only a node that still exists,
        // is still unconnected, and still has a parent in this collection.
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
            // Publish a UUID origin only. A local file or URL identity stays on this device.
            await session.metadataChanged({ origin: validId(node.originId) ? node.originId : node.id });
        } else {
            node.connection = { ...binding, phase: 'waiting' };
            await db.putNodes([node]);
        }
    });
    if (!node.connection) return;
    // The manifest entry carries the document key. Other devices read it from the encrypted root replica.
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
/**
 * Starts the transport of the scope of a connected node, then renews the activity leases of the scope.
 * Checkpoints and activity leases expire on the service after 30 days without renewal.
 * A renewal runs at most once in 24 hours for each scope, and one at a time. Concurrent callers share it.
 * It covers the whole folder subtree, stops when the tab hides or the page unloads, and records its
 * time only when every target succeeds. A failure shows a toast and leaves local copies unchanged.
 * The call does nothing for a hidden tab, an ended connection, or an unsupported Live link.
 */
export async function opened(id: string) {
    await initialize();
    const node = app.node(id);
    if (!node?.connection || node.connection.phase === 'ended' || document.hidden) return;
    const scope = rootFor(node);
    if (!scope || (scope.live && !validLinkWrappingKey(scope.live))) return;
    start(scope);
    if (scope.lastRenewed && Date.now() - scope.lastRenewed < 86400000) return;
    if (renewing.has(scopeId(scope))) return renewing.get(scopeId(scope));
    // The root goes first. The service does not renew a folder whose root lease is not renewed.
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
                // A folder below the root has no replica of its own, only a lease.
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
                // Renew the newest checkpoint of each writer and stream, then the document lease.
                // Keys sort newest first within a stream, and a newer checkpoint contains the earlier ones.
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
// An expanded synced folder renews when the page becomes visible or is shown again. The reader
// does not need to open it a second time.
const renewVisibleFolders = () => {
    if (!document.hidden)
        for (const scope of scopes.values())
            if (scope.kind === 'folder' && app.node(scope.localRoot)?.open)
                void opened(scope.localRoot).catch(() => {});
};
document.addEventListener('visibilitychange', renewVisibleFolders);
window.addEventListener('pageshow', renewVisibleFolders);
/**
 * Undoes or redoes the last local body edit of a connected document, shows the result, and requests
 * an upload. Edits from other participants stay. Throws `SyncError` 403 without edit permission.
 */
export async function undo(id: string, redo = false) {
    const session = sessions.get(id);
    if (!session) return;
    const text = await session.undo(redo);
    await persistProjection(id, text);
    void driverFor(rootFor(app.node(id)!))?.upload();
}
/**
 * Publishes a signed acknowledgement that this participant reviewed the current body.
 * Other devices of the participant then clear their review marker for the same body.
 * A member with view permission publishes nothing.
 */
export async function markReviewed(id: string) {
    const session = sessions.get(id);
    if (!session) return;
    await session.markReviewed();
    void driverFor(rootFor(app.node(id)!))?.upload();
}
/**
 * Reverts a review action of this member: a deleted thread, a deleted comment, or a resolve toggle.
 * Throws `SyncError` 409 when this member has no matching deletion to cancel.
 * @param before The threads to restore.
 * @param after The current threads.
 */
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
/**
 * Publishes a reading width as shared metadata of a connected document, for every participant.
 * `width` is a number or `'full'`. Throws `SyncError` 403 without edit permission.
 */
export async function sharedWidth(id: string, width: number | 'full') {
    const session = sessions.get(id);
    if (!session) return;
    await session.metadataChanged({ readingWidth: width });
    const node = app.node(id)!;
    node.readingWidth = width;
    void driverFor(rootFor(node))?.upload();
}
/**
 * Verifies and decrypts a key update for a Live link, and returns the document key and its epoch.
 * The update arrives through the service. It must carry the owner's signature, and it must decrypt
 * with the wrapping key of the link, which no request carries.
 * Throws when the signature is invalid, when decryption fails, or when the link has no independent
 * wrapping key. The caller verifies `owner` against the owner key of the link first.
 */
export async function readLiveKey(
    update: { value: { document: string; epoch: number; nonce: string; ciphertext: string }; signature: string },
    link: LiveLink,
    owner: SignedGrant,
) {
    if (!(await verify(update.value, update.signature, owner.grant.publicKey)))
        throw new Error('Invalid key update signature');
    return openJson<{ epoch: number; key: string }>(update.value, linkWrappingKey(link), { invitation: link.id });
}
/**
 * Reads the document key of `epoch` from the service and installs it in the session.
 * The service response is untrusted. A wrapped key counts only when it names this document, epoch,
 * and member, and carries the owner's signature. A Live recipient can also take the key update of its link.
 * Throws when no valid key for `epoch` exists. Local work stays in the outbox.
 * Side effects: the session takes the key and seals its outbox again. The tree node takes the epoch,
 * and the scope saves a new root key. With `staged`, only the session and the in-memory scope change.
 */
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
    // A Live recipient reads the key update that the owner encrypted for its link.
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
/**
 * Resolves a draft conflict when the user asks: saves the open editor draft as an independent local
 * document, discards the draft, applies the waiting remote checkpoint, and opens the copy.
 */
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
