import * as app from '../app.ts';
import * as db from '../store/db.ts';
import * as viewer from '../doc/viewer.js';
import { ancestors, descendants, newId, nextOrder, textBytes, type TreeNode } from '../tree/model.ts';
import { esc, modal, copyRich } from '../ui/ui.ts';
import { mergeDocuments, openThreads, splitComments } from '../share/merge.ts';
import { matchScore } from '../share/match.ts';
import { encodeState, decodeState } from '../share/codec.ts';
import { identity, keyBytes, randomSecret, tokenHash, sign, signGrant, wrapKey, unwrapKey, verify, verifyGrant, pairingPhrase, sealJson, openJson, openCheckpoint, encryptRecovery, decryptRecovery, hash, MAX_RECOVERY_FILE_BYTES } from './crypto.ts';
import { encode, decode, canonical, jsonBytes, LIMITS, RejectedPacket, SyncError, validId, validGrant, validCheckpoint, validInvitation, validWrappedKey, checkpointKey, type SignedGrant, type SignedInvitation, type WrappedKey, type SyncBinding, type SyncDirectory } from './protocol.ts';
import { Session, stagedSession, type SessionRecord, type ManifestNode } from './session.ts';
import { materializable, materializationVersion } from './materialization.ts';
import { Transport, type ConnectionStatus } from './transport.ts';
import { BodyReplica } from './replica.ts';
import { resumeRotation, validateRotationLinks, type RotationPlan, type RotationDocument } from './rotation.ts';
import { encryptRecoveryArchive, decryptRecoveryArchive, isRecoveryArchive, MAX_RECOVERY_ARCHIVE_BYTES } from './recovery-archive.ts';
import { SyncDialog } from './dialog.ts';
import { icon, type IconName } from '../ui/icons.ts';
import { copyFeedback } from '../ui/feedback.ts';
import { isShortLiveLink, liveAccess, newLiveSecret, openLiveBootstrap, sealLiveBootstrap, linkWrappingKey, validLinkWrappingKey, type LiveLink } from './live-link.ts';
import { unzipSync } from 'fflate';
import { isShortSyncLink, newSyncSecret, openSyncBootstrap, sealSyncBootstrap, syncAccess, type PairLink } from './sync-link.ts';
import { syncDebug, safeError, type SetupStage } from './debug.ts';
import { connectionStates } from './status.ts';

interface Scope {
    collection: string; root: string; localRoot: string; kind: 'doc' | 'folder'; key: string; token: string;
    owner: SignedGrant; member: SignedGrant; identity: Awaited<ReturnType<typeof identity>>; exclusions: string[];
    lastRenewed?: number;
    personalSync?: boolean;
    cleanup?: { document: string; after?: string };
    selection?: string[];
    live?: LiveLink;
    liveUrl?: string;
    clientNames?: Record<string, string>;
}
const sessions = new Map<string, Session>(), scopes = new Map<string, Scope>(), drivers = new Map<string, Transport>();
const preparing = new Map<string, Session>();
const renewing = new Map<string, Promise<void>>();
const rotations = new Map<string, RotationPlan>();
const rotationTasks = new Map<string, Promise<void>>();
const cleanupTasks = new Map<string, Promise<void>>();
let initialized = false;
const materializations = new Map<string, Promise<void>>();
const materializedViews = new Map<string, string>();
const creations = new Map<string, Promise<void>>();
const scopeKey = (collection: string, member: string) => `sync:scope:${collection}:${member}`;
const scopeId = (scope: Scope) => scopeKey(scope.collection, scope.member.grant.member);
const rotationKey = (scope: Scope) => `sync:rotation:${scopeId(scope)}`;
interface RevokedAccess { members: string[]; invitations: string[] }
const revokedAccessKey = (scope: Scope) => `sync:revoked-access:${scopeId(scope)}`;
const rotationRepository = (scope: Scope) => ({ get: () => db.getMeta<RotationPlan | null>(rotationKey(scope)), commit: async (expected: number | null, value: RotationPlan | null) => {
    const extras: Record<string, unknown> = {};
    if (!value) {
        const plan = await db.getMeta<RotationPlan>(rotationKey(scope));
        if (plan) {
            const previous = await db.getMeta<RevokedAccess>(revokedAccessKey(scope)) ?? { members: [], invitations: [] };
            extras[revokedAccessKey(scope)] = 'member' in plan.target
                ? { ...previous, members: [...new Set([...previous.members, plan.target.member])] }
                : { ...previous, invitations: [...new Set([...previous.invitations, plan.target.invitation])] };
        }
    }
    await db.commitMeta(rotationKey(scope), expected, value, extras);
    if (value) rotations.set(scopeId(scope), value); else rotations.delete(scopeId(scope));
} });
const belongs = (node: TreeNode, scope: Scope) => node.connection?.collection === scope.collection && node.connection.member === scope.member.grant.member;
const driverFor = (scope?: Scope) => scope ? drivers.get(scopeId(scope)) : undefined;
const errorText = (error: unknown) => (error as Error)?.message ?? String(error);
const rootFor = (node: TreeNode) => node.connection ? scopes.get(scopeKey(node.connection.collection, node.connection.member)) : undefined;
const repository = (id: string) => ({ get: () => db.getSide<SessionRecord>('sync', id), commit: async (expected: number | null, record: SessionRecord, remote = false) => {
    const node = app.node(id); if (!node) throw new Error('Connection was removed');
    const staged = new BodyReplica(decode(record.body)), meta = staged.doc.getMap('meta').toJSON(), body = staged.text(); staged.destroy();
    const previous = node.kind === 'doc' ? await db.getText(id) : record.projection;
    const extras: Record<string, unknown> = {};
    const bodyHash = await hash(new TextEncoder().encode(body));
    const reviewed = Object.values(record.reviews).flatMap((packet) => packet.operations).some((op) => op.operation.kind === 'ack'
        && op.operation.participant === record.member.grant.participant && op.operation.value === bodyHash);
    const changed = remote && previous !== undefined && previous !== '' && app.bodyOf(previous) !== app.bodyOf(record.projection);
    const searchableChanged = previous !== record.projection || typeof meta?.name === 'string' && meta.name !== node.name;
    const updated = { ...node, ...(changed && !node.pending ? { pending: true as const } : {}), ...(typeof meta?.name === 'string' ? { name: meta.name } : {}),
        // Local file/URL identities must survive enrollment and later receipts.
        // The portable UUID in Sync metadata is not the local source location.
        ...(validId(meta?.origin) && (!node.originId || validId(node.originId)) ? { originId: meta.origin } : {}),
        ...((meta.readingWidth === 'full' || typeof meta.readingWidth === 'number') ? { readingWidth: meta.readingWidth as 'full' | number } : {}),
        ...(node.handle && previous !== undefined && previous !== record.projection ? { dirty: true } : {}),
        size: textBytes(record.projection), threads: openThreads(record.projection), updatedAt: searchableChanged ? Date.now() : node.updatedAt };
    if (changed && !node.pending) extras[`base:${id}`] = previous;
    if (reviewed) { delete updated.pending; delete extras[`base:${id}`]; }
    await db.commitReplica(id, expected, record, [updated], node.kind === 'doc' ? { [id]: record.projection } : {}, {}, extras, reviewed ? [`base:${id}`] : [], record.binding);
    Object.assign(node, updated); connectionStates.restore([node]); if (reviewed) delete node.pending; app.notify();
} });
async function saveScope(scope: Scope) { scopes.set(scopeId(scope), scope); await db.setMeta(scopeId(scope), scope); }
async function request(action: string, body: Record<string, unknown>, token: string, signal?: AbortSignal) {
    const started = performance.now();
    try {
    const response = await fetch(`/sync/${action}`, { method: 'POST', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(body), cache: 'no-store', credentials: 'omit', referrerPolicy: 'no-referrer', redirect: 'error', signal });
    syncDebug.record('request.finished', { action, status: response.status, duration: Math.round(performance.now() - started) });
    const result = await response.json(); if (!response.ok) throw new SyncError(result.error ?? 'sync-request-failed', response.status); return result;
    } catch (error) { syncDebug.record('request.failed', { action, duration: Math.round(performance.now() - started), ...safeError(error) }); throw error; }
}
const scoped = (scope: Scope, action: string, values: Record<string, unknown> = {}, signal?: AbortSignal) => request(action, { collection: scope.collection, member: scope.member.grant.member, ...values }, scope.token, signal);
async function waitForPair(action: string, values: Record<string, unknown>, token: string, controller: AbortController, ready: (value: Record<string, unknown>) => boolean) {
    let read: AbortController | null = null;
    const pause = () => { if (document.hidden || !navigator.onLine || controller.signal.aborted) read?.abort(); };
    const leave = () => controller.abort();
    document.addEventListener('visibilitychange', pause); window.addEventListener('offline', pause);
    window.addEventListener('pagehide', leave); controller.signal.addEventListener('abort', pause);
    try {
        while (!controller.signal.aborted) {
            if (!document.hidden && navigator.onLine) {
                read = new AbortController();
                try { const result = await request(action, values, token, read.signal); if (!controller.signal.aborted && !read.signal.aborted && ready(result)) return result; }
                catch (error) { if ((error as Error)?.name !== 'AbortError') throw error; }
                finally { read = null; }
            }
            await new Promise((resolve) => setTimeout(resolve, 1000));
        }
    } finally {
        read?.abort(); document.removeEventListener('visibilitychange', pause); window.removeEventListener('offline', pause);
        window.removeEventListener('pagehide', leave); controller.signal.removeEventListener('abort', pause);
    }
}
function phase(scope: Scope, state: ConnectionStatus) {
    syncDebug.record('transport.status', { state });
    if (rotations.has(scopeId(scope))) state = 'attention';
    connectionStates.set({ collection: scope.collection, member: scope.member.grant.member }, state);
    for (const node of app.state.nodes) if (belongs(node, scope)) node.connection!.phase = state;
    if (state === 'ended') void db.markConnectionEnded(scope.collection, scope.member.grant.member).catch(() => {});
    viewer.refreshConnection(); app.notify();
}
async function createSession(node: TreeNode, record: SessionRecord) {
    record.binding.participant = record.member.grant.participant;
    record.binding.ownerParticipant = record.owner.grant.participant;
    node.connection = { ...record.binding, phase: 'waiting' };
    if (node.handle) node.manualFileSave = true;
    await db.commitReplica(node.id, null, record, [node], node.kind === 'doc' ? { [node.id]: record.projection } : {});
    const session = new Session(record, repository(node.id)); sessions.set(node.id, session); return session;
}
async function persistProjection(id: string, text: string) {
    if (id !== app.state.currentId) return;
    if (text === viewer.currentText()) { sessions.get(id)?.present(text); return; }
    if (viewer.applyConnectionProjection({ text, name: app.node(id)!.name, readingWidth: app.node(id)!.localReadingWidth ?? app.node(id)!.readingWidth })) sessions.get(id)?.present(text);
}
function start(scope: Scope) {
    if (scope.live && !validLinkWrappingKey(scope.live)) { driverFor(scope)?.dispose(); drivers.delete(scopeId(scope)); phase(scope, 'attention'); return; }
    if (drivers.has(scopeId(scope))) return;
    const list = () => [...sessions.entries()].filter(([, session]) => session.record.binding.collection === scope.collection && session.record.binding.member === scope.member.grant.member && !session.record.ended);
    const adapter = {
        binding: { collection: scope.collection, document: scope.root, member: scope.member.grant.member, permission: scope.member.grant.permission, epoch: 1, kind: 'sync' as const },
        async prepare() {
            const current = app.node(app.state.currentId);
            if (current && belongs(current, scope) && !viewer.canPublishConnection() || await rotationRepository(scope).get()) return null;
            for (const [, session] of list()) { const envelope = await session.prepare(); if (envelope) { preparing.set(envelope.checkpoint.operation, session); return envelope; } }
            return null;
        },
        async pending() { let count = 0; for (const [, session] of list()) count += await session.pending(); return count; },
        async acknowledge(operation: string, digest: string) { const session = preparing.get(operation); if (!session) throw new Error('Unknown acknowledgement'); await session.acknowledge(operation, digest); preparing.delete(operation); },
        async receive() { return null; },
    };
    const driver = new Transport({ origin: location.origin, token: scope.token, ownerPublicKey: scope.owner.grant.publicKey, replica: adapter,
        exclude: () => scope.exclusions,
        streams: ['body', 'review', 'tree'], collectionScope: scope.kind === 'folder' || (scope.selection?.length ?? 0) > 1, authorValid: (author) => verifyGrant(author, scope.owner.grant),
        directory: scope.kind === 'folder',
        wanted: key => { const checkpoint = checkpointKey(key); return !checkpoint || list().some(([, session]) => session.record.binding.document === checkpoint.document); },
        known: async key => {
            const checkpoint = checkpointKey(key); if (!checkpoint || checkpoint.collection !== scope.collection) return false;
            const entry = list().find(([, session]) => session.record.binding.document === checkpoint.document); if (!entry) return false;
            const applied = (record?: SessionRecord) => !!record && !record.ended && record.binding.collection === scope.collection && record.binding.member === scope.member.grant.member
                && record.binding.document === checkpoint.document && (record.progress[`${checkpoint.member}/${checkpoint.stream}`] ?? 0) >= checkpoint.sequence;
            // Session.record is a committed snapshot. Only consult storage when
            // another tab might have received a packet this tab has not seen.
            return applied(entry[1].record) || applied(await entry[1].repository.get());
        },
        status: (state) => phase(scope, state), projection: async () => {},
        keyChanged: async (envelope, signal) => {
            const session = list().find(([, item]) => item.record.binding.document === envelope.checkpoint.document)?.[1];
            if (!session) return;
            const { epoch } = await scoped(scope, 'epoch', { document: envelope.checkpoint.document }, signal);
            await refreshKey(scope, session, epoch, signal);
            preparing.delete(envelope.checkpoint.operation);
        },
        receive: async (envelope, author, signal, directory) => {
            const entry = list().find(([, session]) => session.record.binding.document === envelope.checkpoint.document);
            if (!entry) return null;
            const [id, session] = entry;
            if (!app.node(id)?.connection || scope.exclusions.includes(envelope.checkpoint.document)) return null;
            if (envelope.checkpoint.epoch > session.record.binding.epoch) await refreshKey(scope, session, envelope.checkpoint.epoch, signal);
            const draft = id === app.state.currentId ? viewer.connectionDraft() : null;
            if (draft && envelope.checkpoint.stream === 'body') {
                const staged = new BodyReplica(session.presented.checkpoint());
                const anchor = staged.anchor(draft.start, draft.end);
                try {
                    const key = session.record.keys?.[String(envelope.checkpoint.epoch)] ?? session.record.key;
                    try { staged.apply(await openCheckpoint(envelope, decode(key), author.grant)); }
                    catch (error) { throw new RejectedPacket(error); }
                    if (!staged.unchanged(anchor)) viewer.setConnectionDraftConflict(() => { void recoverDraft(id, session, envelope, author); });
                } finally { staged.destroy(); }
                return null;
            }
            if (id === app.state.currentId && !viewer.canApplyConnection()) return null;
            if (id === app.state.currentId) await viewer.flush();
            const text = await session.receive(envelope, author);
            const meta = session.metadata(), node = app.node(id)!;
            if (typeof meta.name === 'string') node.name = meta.name;
            if (meta.readingWidth === 'full' || typeof meta.readingWidth === 'number') node.readingWidth = meta.readingWidth;
            // Any editor can write `meta.kind` in the shared replica. Only the root of a folder scope
            // carries a manifest, so decide from the local binding, not from received content.
            if (scope.kind === 'folder' && session.record.binding.document === scope.root && session.folder()) await materialize(scope, session, signal, { directory });
            await persistProjection(id, text); return text;
        }, cycle: async (signal, directory) => {
            if (scope.kind === 'folder') {
                directory ??= await scoped(scope, 'directory', {}, signal);
                const root = sessions.get(scope.localRoot);
                // A previous download can fail after its manifest was saved.
                // Recover from that durable manifest even when no packet needs fetching.
                if (root) { await root.synchronize(); if (materializedViews.get(scopeId(scope)) !== materializationVersion(scope.localRoot, root.record.body, scope.exclusions, directory!)) await materialize(scope, root, signal, { directory }); }
                await reconcileDirectory(scope, signal, directory);
            }
            const current = sessions.get(app.state.currentId ?? '');
            if (current?.record.binding.collection === scope.collection && viewer.canApplyConnection()) await persistProjection(app.state.currentId!, current.record.projection);
        } });
    drivers.set(scopeId(scope), driver); driver.start();
}
export async function initialize(force = false) {
    if (initialized && !force) return; initialized = true;
    if (await db.repairDuplicateConnections()) await app.load();
    const records = await db.allSides<SessionRecord>('sync');
    for (const record of records) {
        if (record?.kind !== 'connected') continue;
        const node = app.state.nodes.find((n) => n.connection?.document === record.binding.document && n.connection.collection === record.binding.collection && n.connection.member === record.binding.member);
        if (!node) continue;
        const scope = await db.getMeta<Scope>(scopeKey(record.binding.collection, record.binding.member))
            ?? await db.getMeta<Scope>(`sync:scope:${record.binding.collection}`);
        if (scope?.member.grant.member !== record.binding.member) continue;
        if (!scope) continue;
        const active = scopes.get(scopeId(scope)); if (active) Object.assign(active, scope); else scopes.set(scopeId(scope), scope);
        if (!sessions.has(node.id)) sessions.set(node.id, new Session(record, repository(node.id)));
    }
    for (const [id, session] of sessions) if (!app.node(id)?.connection) { session.destroy(); sessions.delete(id); }
    for (const scope of scopes.values()) {
        const wasRotating = rotations.has(scopeId(scope));
        const rotation = await rotationRepository(scope).get();
        if (rotation) { rotations.set(scopeId(scope), rotation); if (!rotationTasks.has(scopeId(scope))) driverFor(scope)?.stop(); phase(scope, 'attention'); }
        else rotations.delete(scopeId(scope));
        if (scope.cleanup) { void continueCleanup(scope).catch(() => phase(scope, 'attention')); continue; }
        if (!rotation && [...sessions.values()].some((s) => s.record.binding.collection === scope.collection && s.record.binding.member === scope.member.grant.member && !s.record.ended)) start(scope);
        if (!rotation && wasRotating) driverFor(scope)?.retry();
        if (!document.hidden && scope.kind === 'folder' && app.node(scope.localRoot)?.open) void opened(scope.localRoot).catch(() => {});
    }
}
export async function refresh(ids: string[]) {
    await initialize(true);
    for (const [id, session] of sessions) {
        const node = app.node(id);
        if (!node?.connection) { session.destroy(); sessions.delete(id); continue; }
        if (!ids.includes(id)) continue;
        const text = await session.synchronize();
        if (id === app.state.currentId && viewer.canApplyConnection()) await persistProjection(id, text);
    }
    for (const scope of scopes.values()) {
        const stored = await db.getMeta<Scope>(scopeId(scope));
        if (stored) Object.assign(scope, stored);
        if (!app.state.nodes.some((node) => belongs(node, scope))) { driverFor(scope)?.dispose(); drivers.delete(scopeId(scope)); }
    }
    viewer.refreshConnection();
}
export async function present(id: string) {
    await initialize(); const session = sessions.get(id); if (!session) return;
    await persistProjection(id, await session.synchronize());
}
export async function snapshot(node: TreeNode, text: string) {
    await initialize(); const session = sessions.get(node.id); if (!session) throw new Error('This replica is not ready');
    await validateSnapshot(session, text);
    await persist(node, text);
}
async function validateSnapshot(session: Session, text: string) {
    let record = structuredClone(await session.repository.get());
    if (!record) throw new Error('This replica is unavailable');
    const staged = new Session(record, { get: async () => structuredClone(record), commit: async (_expected, next) => { record = structuredClone(next); } });
    try { await staged.change(record.projection, text); }
    finally { staged.destroy(); }
}
class SyncSetupError extends Error {
    readonly code: string;
    readonly stage: SetupStage;
    constructor(stage: SetupStage, cause: unknown) {
        super('Sync setup did not finish', { cause }); this.name = 'SyncSetupError'; this.stage = stage; this.code = safeError(cause).code;
        syncDebug.record('setup.failed', { stage, ...safeError(cause) }, true);
    }
}
function unfinishedScope(snapshot: db.UnfinishedSetupSnapshot, record: SessionRecord) {
    return (snapshot.metadata[scopeKey(record.binding.collection, record.binding.member)] ?? snapshot.metadata[`sync:scope:${record.binding.collection}`]) as Scope | undefined;
}
function canRestartUnfinished(snapshot: db.UnfinishedSetupSnapshot, id: string) {
    const records = Object.values(snapshot.records) as SessionRecord[];
    return snapshot.nodes[0]?.id === id && snapshot.nodes[0].kind === 'folder' && !snapshot.nodes.some(node => node.connection) && records.length > 0
        && records.every(record => {
            if (record?.kind !== 'connected' || !record.binding || !validId(record.binding.collection) || !validId(record.binding.member)) return false;
            const scope = unfinishedScope(snapshot, record);
            return scope?.kind === 'folder' && scope.localRoot === id && scope.collection === record.binding.collection
                && validGrant(scope.member) && validGrant(scope.owner) && scope.member.grant.member === record.binding.member
                && scope.member.grant.document === null;
        });
}
// Convert the preserved raw snapshot only on explicit backup restoration. Current
// text and the old CRDT/outbox become separate copies, so neither can replace the other.
function recoveryFromUnfinished(saved: db.SavedUnfinishedSetup) {
    const snapshot = saved.snapshot;
    if (saved.v !== 1 || saved.kind !== 'mars-unfinished-sync' || !Array.isArray(snapshot?.nodes) || !snapshot.records || !snapshot.metadata || !snapshot.texts) throw new Error('Invalid unfinished setup backup');
    const grouped = new Map<string, { scope: Scope; nodes: TreeNode[]; records: { local: string; record: SessionRecord }[] }>();
    const preservedProjections: { node: TreeNode; text: string }[] = [];
    const records = Object.entries(snapshot.records).sort(([a, x], [b, y]) => Number(b === unfinishedScope(snapshot, y as SessionRecord)?.localRoot) - Number(a === unfinishedScope(snapshot, x as SessionRecord)?.localRoot));
    for (const [id, value] of records) {
        const record = value as SessionRecord, node = snapshot.nodes.find(node => node.id === id);
        if (!node || record?.kind !== 'connected' || !record.binding) throw new Error('Invalid unfinished setup replica');
        const scope = unfinishedScope(snapshot, record); if (!scope) throw new Error('Missing unfinished setup credentials');
        let group = grouped.get(scopeId(scope));
        if (!group) { group = { scope: structuredClone(scope), nodes: [], records: [] }; grouped.set(scopeId(scope), group); }
        // Repair archives can include the old competing replicas. Restore one
        // active binding, and retain every other projection as a local copy.
        const duplicate = group.nodes.find(item => item.connection!.document === record.binding.document);
        if (duplicate) {
            preservedProjections.push({ node: { ...node, id: newId(), parentId: null, connection: undefined, name: `${node.name} (preserved Sync work)` }, text: record.projection });
            continue;
        }
        const { parentId, kind, name, order, readingWidth } = node;
        group.nodes.push({ id, parentId, kind, name, order, readingWidth, createdAt: saved.at, updatedAt: saved.at, connection: { ...record.binding, phase: 'waiting' } });
        group.records.push({ local: id, record });
    }
    const connections = [...grouped.values()].map(group => {
        const originalScope = structuredClone(group.scope), root = group.nodes.find(node => node.id === group.scope.localRoot);
        if (!root || group.scope.selection) {
            const first = group.nodes.find(node => node.kind === 'doc'), record = group.records.find(item => item.local === first?.id)?.record;
            if (!first || !record) throw new Error('Missing unfinished setup document');
            group.scope.root = first.connection!.document; group.scope.localRoot = first.id; group.scope.kind = 'doc'; group.scope.key = record.key;
            group.scope.selection = group.nodes.filter(node => node.kind === 'doc').map(node => node.connection!.document);
        }
        const ids = new Set(group.nodes.map(node => node.id));
        for (const node of group.nodes) node.parentId = node.id === group.scope.localRoot ? null : ids.has(node.parentId ?? '') ? node.parentId : group.scope.kind === 'folder' ? group.scope.localRoot : null;
        const documents = new Set(group.nodes.map(node => node.connection!.document));
        const links = (snapshot.metadata[`sync:links:${scopeId(originalScope)}`] ?? []) as OwnedLink[];
        return { ...group, links: links.filter(link => documents.has(link.document)), rotation: snapshot.metadata[rotationKey(originalScope)] as RotationPlan | null | undefined, revoked: snapshot.metadata[revokedAccessKey(originalScope)] as RevokedAccess | undefined };
    });
    const ids = new Map(snapshot.nodes.map(node => [node.id, newId()]));
    const local = [...snapshot.nodes.map(node => ({ node: { id: ids.get(node.id)!, parentId: ids.get(node.parentId ?? '') ?? null, kind: node.kind, name: node.name, order: node.order, readingWidth: node.readingWidth }, text: node.kind === 'doc' ? snapshot.texts[node.id] : null })), ...preservedProjections];
    for (const group of connections) {
        const ids = new Map(group.nodes.map(node => [node.id, newId()]));
        group.scope.localRoot = ids.get(group.scope.localRoot)!;
        group.nodes = group.nodes.map(node => ({ ...node, id: ids.get(node.id)!, parentId: ids.get(node.parentId ?? '') ?? null }));
        group.records = group.records.map(item => ({ ...item, local: ids.get(item.local)! }));
    }
    return { v: 3, connections, local };
}
async function enrollSelection(id: string, personalSync = true, previous?: db.UnfinishedSetupSnapshot) {
    let stage: SetupStage = 'initializing', startedScope: Scope | undefined;
    const advance = (next: SetupStage, values: Record<string, unknown> = {}) => { stage = next; syncDebug.record('setup.stage', { stage, ...values }); };
    try {
    advance('initializing', { temporary: db.isTemporaryStorage(), online: navigator.onLine, hidden: document.hidden });
    await initialize(); const node = app.node(id); if (!node) throw new db.ReplicaConflict('missing-node');
    if (node.connection) throw new db.ReplicaConflict('already-connected');
    if (db.isTemporaryStorage()) throw new Error('temporary-storage');
    advance('saving-edits');
    await viewer.flush();
    advance('checking-selection');
    const selected = structuredClone(previous?.nodes ?? [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])]);
    const stored = new Map((await db.allNodes()).map(item => [item.id, item]));
    const storedConnected = selected.filter(item => stored.get(item.id)?.connection).length;
    const orphanRecords = (await Promise.all(selected.filter(item => !stored.get(item.id)?.connection).map(item => db.getSide('sync', item.id)))).filter(Boolean).length;
    syncDebug.record('setup.stage', { stage, items: selected.length, documents: selected.filter(item => item.kind === 'doc').length, folders: selected.filter(item => item.kind === 'folder').length, connected: selected.filter(item => item.connection).length, storedConnected, orphanRecords });
    if (selected.some((n) => n.connection) || storedConnected) throw new db.ReplicaConflict('already-connected');
    if (selected.length > LIMITS.documents) throw new Error('selection-too-large');
    if (orphanRecords && !previous) throw new db.ReplicaConflict('orphan-replica');
    if (previous && !canRestartUnfinished(previous, id)) throw new Error('unfinished-setup-unavailable');
    const who = await identity(), token = randomSecret(), root = newId(), key = encode(keyBytes()), collection = newId();
    const grant = { v: 1 as const, collection, member: newId(), participant: newId(), permission: 'owner' as const, document: null,
        publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(token), expires: 0 };
    const owner = await signGrant(grant, who), scope: Scope = { collection, root, localRoot: id, kind: node.kind, key, token, owner, member: owner, identity: who, exclusions: [], personalSync, lastRenewed: Date.now() };
    startedScope = scope;
    const ids = new Map(selected.map((n) => [n.id, n.id === id ? root : newId()]));
        advance('reading-documents');
        const texts: Record<string, string> = previous ? { ...previous.texts } : {};
        for (const item of selected) if (item.kind === 'doc') texts[item.id] = previous ? previous.texts[item.id] : await app.docText(item.id);
        advance('creating-collection');
        await request('create', { owner, root: { document: root, kind: node.kind } }, token);
        const values: ManifestNode[] = [], records: Record<string, SessionRecord> = {}, connected = structuredClone(selected);
        for (const [index, item] of selected.entries()) {
            advance('registering-items', { item: index + 1, items: selected.length });
            const remote = ids.get(item.id)!, docKey = item.id === id ? key : encode(keyBytes());
            await scoped(scope, 'document', { document: remote, kind: item.kind });
            const binding: SyncBinding = { collection, document: remote, member: grant.member, permission: 'owner', kind: 'sync', epoch: 1, root };
            binding.participant = grant.participant; binding.ownerParticipant = grant.participant;
            const target = connected.find(node => node.id === item.id)!;
            target.connection = { ...binding, phase: 'waiting' }; if (target.handle) target.manualFileSave = true;
            if (item.kind === 'doc' || item.id === id) {
                let record = Session.seed(binding, texts[item.id] ?? '', who, token, docKey, owner, item.name);
                const session = new Session(record, { get: async () => structuredClone(record), commit: async (_expected, next) => { record = structuredClone(next); } });
                try {
                    await session.metadataChanged({ kind: item.kind, origin: validId(item.originId) ? item.originId : item.id, ...(item.readingWidth !== undefined ? { readingWidth: item.readingWidth } : {}) });
                    await session.initial(); records[item.id] = record;
                } finally { session.destroy(); }
            }
            values.push({ id: remote, kind: item.kind, parent: item.id === id ? null : ids.get(item.parentId!) ?? root, name: item.name, order: item.order,
                ...(item.kind === 'doc' ? { key: docKey, origin: item.originId && !/^(file|url):/.test(item.originId) ? item.originId : item.id } : {}) });
        }
        advance('preparing-manifest');
        if (scope.kind === 'folder') {
            const session = new Session(records[id], { get: async () => structuredClone(records[id]), commit: async (_expected, next) => { records[id] = structuredClone(next); } });
            try { await session.editManifest(values); } finally { session.destroy(); }
        }
        advance('saving-connections');
        await app.serializePersistence(async () => {
            await db.enrollConnected(selected, texts, connected, records, { [scopeId(scope)]: scope }, previous);
            scopes.set(scopeId(scope), scope);
            for (const item of connected) {
                const current = app.node(item.id); if (current) { current.connection = item.connection; if (item.manualFileSave) current.manualFileSave = true; }
                if (records[item.id]) { sessions.get(item.id)?.destroy(); sessions.set(item.id, new Session(records[item.id], repository(item.id))); }
            }
        });
        advance('uploading');
        start(scope); const driver = driverFor(scope)!; await driver.upload();
        if (await driver.options.replica.pending()) throw driver.lastFailure ?? new Error('sync-pending');
        viewer.refreshConnection(); app.notify();
        syncDebug.record('setup.finished', { items: selected.length });
    } catch (error) {
        if (startedScope && scopes.has(scopeId(startedScope))) phase(startedScope, 'attention');
        throw new SyncSetupError(stage, error);
    }
}
async function loadPackets(scope: Scope, session: Session, signal?: AbortSignal, staged = false) {
    const info = await scoped(scope, 'document-info', { document: session.record.binding.document }, signal);
    const stored = await session.repository.get();
    if (stored) { stored.creator = info.creator; await session.repository.commit(stored.revision, { ...stored, revision: stored.revision + 1 }); session.record = { ...stored, revision: stored.revision + 1 }; }
    const keys: string[] = []; let after: string | undefined;
    do {
        const page = await scoped(scope, 'index', { document: session.record.binding.document, after }, signal);
        keys.push(...page.entries.map((e: { key: string }) => e.key)); if (page.complete) break; after = page.cursor;
    } while (after);
    const priority = (key: string) => key.includes(`/${scope.owner.grant.member}/body/`) || key.includes(`/${scope.owner.grant.member}/tree/`) ? 0 : key.includes(`/${info.creator}/review/`) ? 1 : 2;
    keys.sort((a, b) => priority(a) - priority(b));
    for (const key of keys) {
        const packet = await scoped(scope, 'get', { key }, signal);
        // The service response is untrusted input. A malformed or unverifiable checkpoint is skipped,
        // so one bad upload from another participant cannot block a join.
        if (!validCheckpoint(packet.envelope) || !validGrant(packet.author)) continue;
        if (packet.envelope.checkpoint.epoch > session.record.binding.epoch) await refreshKey(scope, session, packet.envelope.checkpoint.epoch, signal, staged);
        try { await session.receive(packet.envelope, packet.author); }
        catch (error) { if (!(error instanceof RejectedPacket)) throw error; }
    }
    session.present(session.record.projection);
}
async function materialize(scope: Scope, rootSession: Session, signal?: AbortSignal, options: { bootstrap?: TreeNode; progress?: (completed: number, total: number) => void; directory?: SyncDirectory } = {}) {
    const { bootstrap, progress } = options;
    const previous = materializations.get(scopeId(scope)) ?? Promise.resolve();
    const task = previous.catch(() => {}).then(async () => {
        signal?.throwIfAborted();
        await rootSession.synchronize();
        const body = rootSession.record.body, manifest = rootSession.manifest();
        const directory = options.directory ?? await scoped(scope, 'directory', {}, signal);
        const version = materializationVersion(scope.localRoot, body, scope.exclusions, directory);
        const registered = new Map<string, { kind: string; deleted: boolean }>(directory.documents.map((item: { id: string; kind: string; deleted: boolean }) => [item.id, item]));
        const values = materializable(scope.root, manifest, registered, scope.exclusions);
        const current = (await db.allNodes()).filter(node => belongs(node, scope));
        const mapping = new Map(current.map(node => [node.connection!.document, node]));
        if (!bootstrap && mapping.size === current.length && mapping.get(scope.root)?.id === scope.localRoot && values.every(item => {
            const node = mapping.get(item.id), parent = mapping.get(item.parent!);
            return node?.kind === item.kind && parent?.kind === 'folder' && node.parentId === parent.id && node.name === item.name && node.order === (Number.isSafeInteger(item.order) ? item.order : 0);
        })) { materializedViews.set(scopeId(scope), version); return; }
        const entries: { node: TreeNode; parent: string; record?: SessionRecord }[] = [];
        const total = values.filter(item => item.kind === 'doc' && !mapping.has(item.id)).length; let completed = 0;
        progress?.(completed, total);
        for (const item of values) {
            const existing = mapping.get(item.id), local = existing?.id ?? newId();
            const node: TreeNode = { id: local, parentId: null, kind: item.kind, name: item.name, order: Number.isSafeInteger(item.order) ? item.order : 0,
                createdAt: Date.now(), updatedAt: Date.now(), originId: item.origin ?? local,
                connection: { collection: scope.collection, document: item.id, member: scope.member.grant.member, permission: scope.member.grant.permission, kind: 'sync', epoch: item.epoch ?? 1, root: scope.root, phase: 'working' } };
            const entry = { node, parent: item.parent! } as typeof entries[number]; entries.push(entry);
            if (!existing && item.kind === 'doc') {
                const record = Session.seed(node.connection!, '', scope.identity, scope.token, item.key!, scope.owner, item.name, scope.member);
                const empty = new BodyReplica();
                record.body = encode(empty.checkpoint()); record.binding.participant = scope.member.grant.participant; record.binding.ownerParticipant = scope.owner.grant.participant; empty.destroy();
                const staged = stagedSession(record);
                try {
                    await loadPackets(scope, staged, signal, true); entry.record = staged.record;
                    const meta = staged.metadata();
                    Object.assign(node, { size: textBytes(staged.record.projection), threads: openThreads(staged.record.projection),
                        ...(validId(meta.origin) ? { originId: meta.origin } : {}),
                        ...(meta.readingWidth === 'full' || typeof meta.readingWidth === 'number' ? { readingWidth: meta.readingWidth } : {}) });
                    node.connection = { ...staged.record.binding, phase: 'working' };
                } finally { staged.destroy(); }
                progress?.(++completed, total);
            }
        }
        signal?.throwIfAborted();
        await app.serializePersistence(async () => {
            signal?.throwIfAborted();
            const root = bootstrap ?? app.node(scope.localRoot); if (!root?.connection) throw new db.ReplicaConflict('missing-node');
            await db.materializeConnected(root, body, entries, bootstrap ? { record: rootSession.record, metadata: { [scopeId(scope)]: scope } } : undefined);
            await app.load();
        });
        materializedViews.set(scopeId(scope), version);
        // Only committed replicas become visible to upload/poll drivers.
        if (bootstrap) scopes.set(scopeId(scope), scope);
        await initialize(true);
    });
    materializations.set(scopeId(scope), task);
    try { await task; } finally { if (materializations.get(scopeId(scope)) === task) materializations.delete(scopeId(scope)); }
}
async function reconcileDirectory(scope: Scope, signal?: AbortSignal, received?: SyncDirectory) {
    const directory = received ?? await scoped(scope, 'directory', {}, signal);
    for (const doc of directory.documents) if (doc.deleted) {
        const node = app.state.nodes.find((n) => belongs(n, scope) && n.connection!.document === doc.id);
        if (node) node.connection!.phase = 'ended';
    }
}
export async function persist(node: TreeNode, text: string) {
    await initialize(); const session = sessions.get(node.id); if (!session) throw new Error('This replica is not ready');
    const next = await session.change(session.record.projection, text); await persistProjection(node.id, next);
    setTimeout(() => { void driverFor(rootFor(node))?.upload(); }, 0);
    const scope = rootFor(node); if (scope && (!scope.lastRenewed || Date.now() - scope.lastRenewed > 86400000)) void opened(node.id);
}
export async function metadata(id: string) {
    await initialize(); const node = app.node(id); if (!node?.connection) return;
    const session = sessions.get(id); if (session) await session.metadataChanged({ name: node.name, ...(node.readingWidth !== undefined ? { readingWidth: node.readingWidth } : {}) });
    const scope = rootFor(node); if (scope?.kind === 'folder') await updateManifest(scope);
    void driverFor(rootFor(node))?.upload();
}
async function updateManifest(scope: Scope) {
    const root = sessions.get(scope.localRoot); if (!root) return;
    const previous = new Map(root.manifest().map((n) => [n.id, n]));
    const values = app.state.nodes.filter((n) => belongs(n, scope)).map((n): ManifestNode => ({
        ...(previous.get(n.connection!.document) ?? {}), id: n.connection!.document, kind: n.kind, name: n.name, order: n.order,
        parent: n.id === scope.localRoot ? null : app.node(n.parentId)?.connection?.document ?? scope.root }));
    await root.editManifest(values);
}
export async function created(id: string) {
    const active = creations.get(id); if (active) return active;
    const task = enrollCreated(id); creations.set(id, task);
    try { await task; } finally { if (creations.get(id) === task) creations.delete(id); }
}
async function enrollCreated(id: string) {
    await initialize(); const node = app.node(id), parent = app.node(node?.parentId ?? null), scope = parent ? rootFor(parent) : undefined;
    if (!node || node.connection || !scope || scope.kind !== 'folder') return;
    const remote = newId(), key = encode(keyBytes()); await scoped(scope, 'document', { document: remote, kind: node.kind });
    const binding: SyncBinding = { collection: scope.collection, document: remote, member: scope.member.grant.member, permission: scope.member.grant.permission, kind: 'sync', epoch: 1, root: scope.root };
    await app.serializePersistence(async () => {
        if (!app.node(id) || app.node(id)?.connection || app.node(node.parentId)?.connection?.collection !== scope.collection) return;
        if (node.kind === 'doc') {
            const record = Session.seed(binding, await app.docText(id), scope.identity, scope.token, key, scope.owner, node.name, scope.member);
            const session = await createSession(node, record); await session.initial();
            await session.metadataChanged({ origin: validId(node.originId) ? node.originId : node.id });
        } else { node.connection = { ...binding, phase: 'waiting' }; await db.putNodes([node]); }
    });
    if (!node.connection) return;
    const root = sessions.get(scope.localRoot)!;
    await root.editManifest([{ id: remote, kind: node.kind, parent: parent!.connection!.document, name: node.name, order: node.order, ...(node.kind === 'doc' ? { key, origin: node.id } : {}) }]);
    if (node.kind === 'folder') for (const child of app.state.nodes.filter((item) => item.parentId === node.id)) await created(child.id);
    void driverFor(scope)?.upload();
}
export async function opened(id: string) {
    await initialize(); const node = app.node(id); if (!node?.connection || node.connection.phase === 'ended' || document.hidden) return;
    const scope = rootFor(node); if (!scope || scope.live && !validLinkWrappingKey(scope.live)) return; start(scope);
    if (scope.lastRenewed && Date.now() - scope.lastRenewed < 86400000) return;
    if (renewing.has(scopeId(scope))) return renewing.get(scopeId(scope));
    const root = app.node(scope.localRoot)!;
    const targets = scope.kind === 'folder' ? [root, ...descendants(app.state.nodes, root.id)].filter(item => belongs(item, scope)) : [node];
    const controller = new AbortController(), pause = () => { if (document.hidden) controller.abort(); }, leave = () => controller.abort();
    document.addEventListener('visibilitychange', pause); window.addEventListener('pagehide', leave);
    const run = (async () => { let complete = true; for (const target of targets) {
        if (controller.signal.aborted) { complete = false; break; }
        const session = sessions.get(target.id);
        try {
            if (!session) { if (target.kind === 'folder') await scoped(scope, 'touch-folder', { document: target.connection!.document }, controller.signal); continue; }
            const entries: { key: string }[] = []; let after: string | undefined;
            do { const page = await scoped(scope, 'index', { document: target.connection!.document, after }, controller.signal); entries.push(...page.entries); if (page.complete) break; after = page.cursor; } while (after);
            const seen = new Set<string>();
            for (const entry of entries) {
                const parts = entry.key.split('/'), group = parts.slice(0, -1).join('/'); if (seen.has(group)) continue; seen.add(group);
                await scoped(scope, 'touch-record', { document: target.connection!.document, key: entry.key }, controller.signal);
            }
            await scoped(scope, 'touch', { document: target.connection!.document }, controller.signal);
        } catch (error) { complete = false; if (!controller.signal.aborted) viewer.toast('Could not extend Sync activity. Your local copies are retained. Try again when the connection is available.'); }
    } if (complete) { scope.lastRenewed = Date.now(); await saveScope(scope); } })();
    renewing.set(scopeId(scope), run);
    try { await run; } finally { document.removeEventListener('visibilitychange', pause); window.removeEventListener('pagehide', leave); renewing.delete(scopeId(scope)); }
}
const renewVisibleFolders = () => { if (!document.hidden) for (const scope of scopes.values()) if (scope.kind === 'folder' && app.node(scope.localRoot)?.open) void opened(scope.localRoot).catch(() => {}); };
document.addEventListener('visibilitychange', renewVisibleFolders); window.addEventListener('pageshow', renewVisibleFolders);
export const enroll = (id: string) => syncDialog(id);
export async function details(id: string) {
    await initialize(); const node = app.node(id);
    if (node?.connection?.kind === 'live') return liveDocument(id);
    return node?.connection?.liveShared ? liveLinks(id) : syncDialog(id);
}
const syncError = (error: unknown) => ({
    'service-unavailable': 'The Sync service is unavailable right now. Your documents are still here. Try again later.',
    'binding-missing': 'This tab lost the saved connection during setup. MARS has not reset your library. Try again.',
    'orphan-replica': 'An earlier setup left saved connection data without a connected item. Your saved documents stay unchanged.',
    'already-connected': 'Some selected items are already connected. Open Sync for those items before connecting the whole folder.',
    'selection-too-large': `This selection has more than ${LIMITS.documents} items. Choose a smaller folder.`,
    'temporary-storage': 'Save this document to your library before turning on Sync.',
    'sync-pending': 'Your connection is saved, but its first upload has not finished. Keep this tab visible and check your connection, then try again.',
    'selection-grown': 'The folder contents changed during setup. Your documents are still here. Try again with the current folder.',
    'missing-node': 'The selected item was removed during setup. Your other documents are still here.',
    'text-changed': 'A document changed during setup. Your latest text is still saved here. Try again.',
    'metadata-changed': 'A name, location, or document setting changed during setup. Your documents are still here. Try again.',
    'revision': 'Another tab changed the saved connection during setup. Your local documents are still here. Try again.',
    'collection-creation-limit': 'Too many new sync connections from this network. Try again later. Your documents stay here.',
    'sync-storage-full': 'Sync storage is full. Your changes are saved here. Try again later.',
    'update-too-large': 'This document is too large to sync. Your changes are saved here. Download a copy to keep them.',
    'invitation-expired': 'This Sync link expired. Go back and create a new one.',
    'unsupported-link-key-format': 'Unsupported link key format. Create a new link from the sharing device.',
}[error instanceof SyncSetupError ? error.code : errorText(error)] ?? 'Could not finish. Your document is still here. Try again.');
const syncTool = (label: string, action: string, glyph: IconName) => `<button type="button" class="btn" data-sync-action="${action}">${icon(glyph)}<span>${label}</span></button>`;
const confirmationCode = (phrase: string) => `<div class="sync-code" role="group" aria-label="Confirmation code">${phrase.split(' ').map(part => `<span>${esc(part)}</span>`).join(' ')}</div>`;
async function syncDialog(id: string) {
    await initialize(); const ui = new SyncDialog(); let openSyncLink = false, problem = '', stopped = false;
    let restart: db.UnfinishedSetupSnapshot | undefined;
    try {
        while (ui.open) {
          try {
            if (restart) { const previous = restart; restart = undefined; ui.busy('Restarting setup…'); await enrollSelection(id, true, previous); problem = ''; openSyncLink = true; continue; }
            const node = app.node(id); if (!node) break;
            const scope = rootFor(node);
            if (node.connection?.kind === 'live') { ui.close(); return liveDocument(id); }
            // Older releases exposed the root before finishing its folder manifest.
            // Restart only after an explicit choice, keeping all saved content local.
            if (scope?.kind === 'folder' && scope.member.grant.permission === 'owner' && id === scope.localRoot && !scope.selection
                && sessions.has(id) && !sessions.get(id)!.manifest().length && !app.state.nodes.some(item => belongs(item, scope) && item.connection!.liveShared)) {
                const answer = await ui.screen({ title: 'Finish folder setup', html: '<p>The previous Sync setup did not finish. Restart setup to include this folder and its documents. Your local documents and comments stay here.</p><p>Devices connected through the incomplete setup will need a new Sync link.</p>',
                    buttons: [{ label: 'Cancel', value: 'close' }, { label: 'Restart setup', value: 'restart', primary: true }] });
                if (answer.value !== 'restart') break;
                ui.busy('Restarting setup…');
                await viewer.flush();
                const selected = [node, ...descendants(app.state.nodes, id)];
                for (const item of selected) if (!item.connection) {
                    // A stale tab in the old release could save an unconnected node
                    // over its binding while leaving the atomic projection/outbox.
                    const record = await db.getSide<SessionRecord>('sync', item.id);
                    if (record?.binding.collection === scope.collection && record.binding.member === scope.member.grant.member) item.connection = { ...record.binding, phase: 'attention' };
                }
                await disconnect(scope, selected.filter(item => belongs(item, scope)));
                await enrollSelection(id);
                problem = ''; openSyncLink = true; continue;
            }
            const personal = scope && (scope.member.grant.permission !== 'owner' || scope.kind === 'folder' || scope.personalSync === true || scope.personalSync === undefined && (!node.connection?.liveShared || Object.keys(scope.clientNames ?? {}).length > 0));
            if (!scope || !personal) {
                if (db.isTemporaryStorage()) { await ui.screen({ title: 'Sync', html: '<p>Save this document to your library before turning on sync.</p>', buttons: [{ label: 'Close', value: 'close' }] }); break; }
                const selection = node.kind === 'folder' ? `this folder and the documents inside it` : 'this document';
                const answer = await ui.screen({ title: 'Sync', html: `${stopped ? '<p>Sync is off. This copy is now independent.</p>' : ''}<p>Keep <strong>${esc(node.name)}</strong> up to date on your other devices.</p><p>Only ${selection} ${node.kind === 'folder' ? 'are' : 'is'} included.</p>${problem ? `<p role="alert">${esc(problem)}</p>` : ''}`,
                    buttons: [{ label: 'Cancel', value: 'close' }, { label: 'Turn on sync', value: 'on', primary: true }] });
                if (answer.value !== 'on') break;
                ui.busy('Turning on Sync…');
                if (scope) { scope.personalSync = true; await saveScope(scope); }
                else await enrollSelection(id);
                if (!app.node(id)?.connection) throw new SyncSetupError('saving-connections', new Error('binding-missing'));
                problem = ''; openSyncLink = true; continue;
            }
            if (openSyncLink && scope.member.grant.permission === 'owner') {
                openSyncLink = false; await syncLinkPanel(scope, ui); if (!ui.open) break;
            }
            const owner = scope.member.grant.permission === 'owner', current = app.node(app.state.currentId);
            const blocked = !!current && belongs(current, scope) && !viewer.canPublishConnection();
            const status = blocked ? 'attention' : node.connection!.phase ?? 'waiting';
            const rotating = !!await rotationRepository(scope).get();
            const label = { current: 'Up to date', working: 'Syncing changes…', waiting: 'Waiting to sync', offline: 'Offline', attention: 'Sync needs attention', ended: 'Sync stopped' }[status];
            const message = blocked ? 'Your latest changes are not saved yet. Try again, or download a copy before closing.'
                : status === 'offline' ? db.isTemporaryStorage() ? 'Your edits are waiting to sync. Keep this tab open or download a copy before closing.' : 'Your changes are saved here and will sync when you are back online.'
                : status === 'ended' ? 'This connection is no longer available. Your document stays here.'
                : status === 'attention' ? 'Your changes are saved here. Try again to finish syncing.'
                : owner ? 'Use a Sync link to connect your other device.' : 'Changes appear on your synced devices.';
            const rootName = app.node(scope.localRoot)?.name ?? node.name;
            const folder = scope.kind === 'folder' && node.id !== scope.localRoot ? `<p>This document syncs with the <strong>${esc(rootName)}</strong> folder. Sync links connect the whole folder.</p>` : '';
            const answer = await ui.screen({ title: 'Sync', html: `<p><strong>${esc(node.name)}</strong></p><p class="sync-status" data-state="${status}">${status === 'working' ? '<span class="su-spinner" aria-hidden="true"></span>' : icon(status === 'current' ? 'check' : status === 'ended' ? 'unlink' : status === 'attention' ? 'warning' : 'refresh')}<strong>${status === 'ended' ? 'Local copy · Sync ended' : label}</strong></p><p>${message}</p>${folder}${problem ? `<p role="alert">${esc(problem)}</p>` : ''}
                <nav class="sync-tools" aria-label="Sync settings">${owner && status !== 'ended' ? syncTool('Devices', 'access', 'refresh') : ''}</nav>
                ${scope.selection ? '<p>To connect a new device, open the original synced folder.</p>' : ''}
                <p class="sync-fineprint">${status === 'ended' ? 'Your local copies remain available.' : scope.kind === 'folder' ? "Sync for this folder and its documents will stop after 30 days without opening the folder or a document inside. Local copies will remain." : "Sync will stop if this document isn't opened for 30 days, but local copies will remain."}</p>`,
                buttons: [{ label: status === 'ended' ? 'Keep as local document' : 'Disconnect this device', value: 'stop', empty: true, left: true },
                    ...(status === 'attention' || status === 'waiting' ? [{ label: 'Try again', value: 'retry', primary: true }] : owner && status !== 'ended' && !rotating && !scope.selection ? [{ label: 'Sync link', value: 'link', primary: true }] : [])] });
            problem = '';
            if (answer.value === 'close') break;
            if (answer.value === 'link') { openSyncLink = true; continue; }
            if (answer.value === 'access') await accessPanel(scope, ui);
            else if (answer.value === 'retry') { ui.busy('Trying again…'); await viewer.flush(); if (scope.cleanup) await continueCleanup(scope, ui.signal); else if (await rotationRepository(scope).get()) await continueRotation(scope); else { start(scope); driverFor(scope)?.retry(); } }
            else if (answer.value === 'stop') {
                const result = await ui.screen({ title: 'Disconnect this device?', back: true, html: `<p>${node.kind === 'folder' ? 'This folder and its documents stay on this device as independent copies.' : 'Your document stays on this device as an independent copy.'} Other devices and active Live links remain connected. This copy will no longer send or receive changes.</p>${node.connection!.liveShared ? '<p>If you also want to stop sharing, disable the Live links first.</p>' : ''}`,
                    buttons: [{ label: 'Disconnect', value: 'stop', primary: true }] });
                if (result.value === 'stop') { ui.busy('Stopping sync…'); await stopHere(id); stopped = true; }
            }
          } catch (error) {
              const details = error instanceof SyncSetupError ? { ...safeError(error), stage: error.stage } : safeError(error);
              if (!(error instanceof SyncSetupError)) syncDebug.record('setup.failed', details, true);
              problem = syncError(error);
              let unfinished: db.UnfinishedSetupSnapshot | undefined;
              if (details.code === 'orphan-replica') {
                  const node = app.node(id);
                  try { if (node) { const snapshot = await db.captureUnfinishedSetup([id, ...descendants(app.state.nodes, id).map(node => node.id)]); if (canRestartUnfinished(snapshot, id)) unfinished = snapshot; } }
                  catch (error) { syncDebug.record('setup.failed', { stage: 'checking-selection', ...safeError(error) }, true); }
              }
              const result = await ui.screen({ title: 'Sync did not finish', html: `<div class="sync-error" role="alert"><p><strong>${esc(problem)}</strong></p><p>This setup attempt did not delete local documents or comments.</p><p class="sync-fineprint">Error code: <code>${esc(details.code)}</code>${'stage' in details ? ` · Step: <code>${esc(details.stage)}</code>` : ''}</p></div>`,
                  buttons: [...(unfinished ? [{ label: 'Restart setup', value: 'restart', primary: true }] : []), { label: 'Back to Sync', value: 'back', primary: !unfinished }] });
              if (result.value === 'close') break;
              if (result.value === 'restart' && unfinished) {
                  const answer = await ui.screen({ title: 'Restart unfinished Sync setup?', back: true, html: '<p>Start a new Sync connection for this folder and its documents.</p><p>Your current documents and comments stay unchanged. Unsent work from the previous setup also stays on this device.</p><p>Devices from the old setup need a new Sync link. Existing saved copies and links are not deleted.</p>', buttons: [{ label: 'Restart setup', value: 'restart', primary: true }] });
                  if (answer.value === 'restart') restart = unfinished;
              }
          }
        }
    }
    finally { ui.close(); }
}
async function stopHere(id: string) {
    const node = app.node(id), scope = node ? rootFor(node) : undefined;
    if (!node || !scope) return;
    await viewer.flush();
    if (viewer.hasConnectionDraftConflict()) throw new Error('Save the draft before stopping sync');
    const nodes = [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])].filter((item) => belongs(item, scope));
    await disconnect(scope, nodes); viewer.refreshConnection(); app.notify();
    if (node.id === app.state.currentId) await app.openDoc(node.id);
}
async function accessPanel(scope: Scope, ui: SyncDialog) {
    while (ui.open) {
        // The member list is a service response. Keep only well-formed grants: their ids reach HTML attributes.
        const listed = (await scoped(scope, 'members')).members;
        const members = (Array.isArray(listed) ? listed : []).filter(validGrant);
        const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope)) ?? { members: [], invitations: [] };
        const devices = members.filter((member) => member.grant.member !== scope.member.grant.member && member.grant.document === null && !member.issuer && !revoked.members.includes(member.grant.member));
        const title = app.node(scope.localRoot)?.name ?? 'this document';
        const deviceRows = devices.map((member, index) => {
            const name = scope.clientNames?.[member.grant.member] ?? `Device ${index + 1}`;
            return `<div class="sync-access-item"><span>${esc(name)}</span><button type="button" class="icon-btn icon-btn-s" data-tooltip="Disconnect device" aria-label="Disconnect ${esc(name)}" data-sync-action="device:${member.grant.member}">${icon('unlink')}</button></div>`;
        }).join('');
        const answer = await ui.screen({ title: 'Sync devices', back: true, html: `<p>Devices syncing <strong>${esc(title)}</strong>.</p><div class="sync-access-list">${deviceRows || '<p>No other devices connected yet. Use a Sync link to connect one.</p>'}</div>` });
        if (answer.value === 'back' || answer.value === 'close') return;
        const id = answer.value.split(':')[1], member = devices.find(member => member.grant.member === id); if (!member) continue;
        const label = scope.clientNames?.[member.grant.member] ?? 'this device';
        const confirm = await ui.screen({ title: 'Disconnect this device?', back: true,
            html: `<p><strong>${esc(label)}</strong> will stop syncing. Local copies and unsent changes stay on that device.</p>`,
            buttons: [{ label: 'Disconnect device', value: 'remove', primary: true }] });
        if (confirm.value === 'close') return;
        if (confirm.value === 'remove') {
            ui.busy('Disconnecting device…'); await revoke(scope, { member: id });
        }
    }
}
export async function liveLinks(id: string) {
    await initialize(); const node = app.node(id), scope = node ? rootFor(node) : undefined;
    if (!node || !scope || scope.member.grant.permission !== 'owner') return;
    const ui = new SyncDialog('liveLinksDialog'); let problem = '';
    try { while (ui.open && node.connection) {
        const pending = !!scope.cleanup || !!await rotationRepository(scope).get();
        const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope)) ?? { members: [], invitations: [] };
        const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? []).filter(link => link.document === node.connection!.document && !revoked.invitations.includes(link.id));
        const rows = links.map((link, index) => `<div class="sync-access-item"><span>${esc(link.label ?? `${node.name} - Link ${index + 1}`)}<small>Can ${esc(link.permission ?? 'view')} · ${link.expires > Date.now() / 1000 ? 'Expires' : 'Expired'} ${new Date(link.expires * 1000).toLocaleDateString()}</small></span><button class="icon-btn icon-btn-s" type="button" aria-label="Disable link ${index + 1}" data-tooltip="Disable Live link" data-sync-action="link:${link.id}">${icon('unlink')}</button></div>`).join('');
        const answer = await ui.screen({ title: 'Live links', html: `<p>Links sharing <strong>${esc(node.name)}</strong>.</p><div class="sync-access-list">${rows || '<p>No active Live links.</p>'}</div><p class="sync-fineprint">Disabling a link stops access through that link. Local copies remain. If this document only uses Live links, removing its last link also deletes the remote copy.</p>${pending ? '<p>A sharing change needs to finish. Try again to continue.</p>' : ''}${problem ? `<p role="alert">${esc(problem)}</p>` : ''}`, buttons: pending ? [{ label: 'Try again', value: 'retry', primary: true }] : [...(links.length ? [{ label: 'Stop sharing', value: 'stop', empty: true, left: true }] : []), { label: 'Copy link', value: 'create', primary: true }] });
        problem = ''; if (answer.value === 'close') break;
        try {
            if (answer.value === 'retry') { ui.busy('Finishing sharing change…'); if (scope.cleanup) await continueCleanup(scope, ui.signal); else await continueRotation(scope); }
            else if (answer.value === 'create') await live(id);
            else if (answer.value === 'stop') await stopSharing(scope, node, ui);
            else {
                const link = links.find(link => link.id === answer.value.split(':')[1]); if (!link) continue;
                const confirm = await ui.screen({ title: 'Disable this link?', back: true, html: '<p>People using this link will lose access to the Live document. Their local copies remain.</p>', buttons: [{ label: 'Disable link', value: 'disable', danger: true }] });
                if (confirm.value !== 'disable') continue;
                ui.busy('Disabling link…'); await revoke(scope, { link });
                if (!await rotationRepository(scope).get()) await finishSharing(scope, link.document, ui.signal);
            }
        } catch (error) { problem = syncError(error); }
    } } finally { ui.close(); }
}
async function liveDocument(id: string) {
    await initialize();
    const ui = new SyncDialog('liveDocumentDialog');
    try { while (ui.open) {
        const node = app.node(id), scope = node ? rootFor(node) : undefined; if (!node?.connection || !scope) break;
        const ended = node.connection.phase === 'ended', access = { view: 'View only', comment: 'Can comment', edit: 'Can edit', owner: 'Owner' }[scope.member.grant.permission];
        const description = scope.live && !validLinkWrappingKey(scope.live) ? 'This link uses an unsupported key format. Ask the owner for a new link. Your local content and pending work stay here.'
            : ended ? 'The Live link ended. Your retained copy and any unsent changes are still here.' : `Receives changes from the sharing source. Access expires ${new Date(scope.member.grant.expires * 1000).toLocaleDateString()}, unless the owner disables the link first.`;
        const answer = await ui.screen({ title: 'Live document', html: `<p><strong>${esc(node.name)}</strong></p><p><strong>${access}</strong></p><p>${description}</p><p class="sync-fineprint">${ended ? 'Keep as a local copy to make this document independent.' : "Keep as a local copy to stop receiving changes without disabling the owner's link."}</p>`, buttons: [{ label: 'Keep as local copy', value: 'local', primary: true }, ...(db.isTemporaryStorage() ? [{ label: 'Save to library', value: 'save' }] : [{ label: 'Remove from library', value: 'remove', empty: true }])] });
        if (answer.value === 'close') break;
        if (answer.value === 'save') { await saveLive(id); continue; }
        const confirm = await ui.screen({ title: answer.value === 'local' ? 'Keep as local copy?' : 'Remove from library?', back: true, html: answer.value === 'local' ? '<p>This document and your current changes stay on this device. This copy will no longer receive Live updates. The owner\'s link stays active for other people.</p>' : '<p>Remove this copy from your library. This does not disable the owner\'s Live link.</p>', buttons: [{ label: answer.value === 'local' ? 'Keep local copy' : 'Remove', value: 'yes', primary: answer.value === 'local', danger: answer.value !== 'local' }] });
        if (confirm.value !== 'yes') continue;
        if (answer.value === 'local') await stopHere(id); else { await remove(id, false); await app.remove(id); }
    } } finally { ui.close(); }
}
function receivedLiveScope(id: string) {
    const node = app.node(id), scope = node ? rootFor(node) : undefined;
    return node?.connection?.kind === 'live' && node.connection.phase !== 'ended' && scope?.live && validLinkWrappingKey(scope.live)
        && scope.member.grant.permission !== 'owner' && scope.member.grant.expires > Date.now() / 1000 ? scope : null;
}
export async function liveLinkInfo(id: string) {
    await initialize(); const scope = receivedLiveScope(id);
    return scope ? { permission: scope.member.grant.permission, expires: scope.member.grant.expires } : null;
}
export async function forwardLiveLink(id: string) {
    await initialize(); const scope = receivedLiveScope(id);
    if (!scope) throw new Error('The Live link ended. Choose Snapshot to share your local copy.');
    try { await request('invite-read', { invitation: scope.live!.id }, scope.live!.secret); }
    catch (error) {
        if (['invitation-expired', 'connection-ended', 'access-ended'].includes(errorText(error))) {
            phase(scope, 'ended'); throw new Error('The Live link ended. Choose Snapshot to share your local copy.');
        }
        throw new Error('Could not confirm the Live link. Check your connection and try again.');
    }
    const url = scope.liveUrl ?? `${location.origin}${location.pathname}#live=${await encodeState(JSON.stringify(scope.live))}`;
    if (!receivedLiveScope(id)) throw new Error('The Live link ended. Choose Snapshot to share your local copy.');
    return { url, expires: scope.member.grant.expires };
}
async function finishSharing(scope: Scope, document: string, signal?: AbortSignal) {
    const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope)) ?? { members: [], invitations: [] };
    const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? []).filter(link => link.document === document && link.expires > Date.now() / 1000 && !revoked.invitations.includes(link.id));
    if (links.length) return;
    const nodes = app.state.nodes.filter(node => belongs(node, scope) && node.connection!.document === document);
    const finish = async () => { for (const node of nodes) delete node.connection!.liveShared; await db.putNodes(nodes); app.notify(); };
    if (scope.personalSync || scope.kind !== 'doc' || scope.root !== document) { await finish(); return; }
    const members = (await scoped(scope, 'members')).members as SignedGrant[];
    if (members.some(member => member.grant.member !== scope.member.grant.member && member.grant.document === null && !revoked.members.includes(member.grant.member))) { await finish(); return; }
    // Keep Live links reachable while cleanup is pending, including after close/reload.
    scope.cleanup = { document }; await saveScope(scope);
    await continueCleanup(scope, signal);
}
async function continueCleanup(scope: Scope, signal?: AbortSignal) {
    if (cleanupTasks.has(scopeId(scope))) return cleanupTasks.get(scopeId(scope));
    const controller = new AbortController(), pause = () => { if (document.hidden) controller.abort(); }, leave = () => controller.abort();
    document.addEventListener('visibilitychange', pause); window.addEventListener('pagehide', leave);
    const task = (async () => {
        driverFor(scope)?.stop();
        while (scope.cleanup) {
            if (document.hidden) throw new DOMException('Cleanup paused', 'AbortError');
            const plan = scope.cleanup;
            let result: { complete: boolean; after?: string };
            try { result = await scoped(scope, 'purge-live-document', plan, signal ? AbortSignal.any([signal, controller.signal]) : controller.signal); }
            catch (error) {
                if (errorText(error) === 'personal-sync-active') {
                    delete scope.cleanup; scope.personalSync = true; await saveScope(scope); driverFor(scope)?.retry(); return;
                }
                if (errorText(error) !== 'connection-expired') throw error;
                result = { complete: true }; // The final cleanup response can be lost after owner metadata is deleted.
            }
            if (result.complete) {
                delete scope.cleanup;
                await disconnect(scope, app.state.nodes.filter(node => belongs(node, scope)));
                viewer.refreshConnection(); app.notify(); viewer.toast('Sharing stopped. The remote copy was deleted. Your local document is retained.'); return;
            }
            scope.cleanup = { document: plan.document, ...(result.after ? { after: result.after } : {}) }; await saveScope(scope);
        }
    })().finally(() => { document.removeEventListener('visibilitychange', pause); window.removeEventListener('pagehide', leave); cleanupTasks.delete(scopeId(scope)); });
    cleanupTasks.set(scopeId(scope), task); return task;
}
const resumeCleanup = () => { if (!document.hidden) for (const scope of scopes.values()) if (scope.cleanup) void continueCleanup(scope).catch(() => phase(scope, 'attention')); };
document.addEventListener('visibilitychange', resumeCleanup); window.addEventListener('pageshow', resumeCleanup);
async function stopSharing(scope: Scope, node: TreeNode, ui: SyncDialog) {
    const answer = await ui.screen({ title: 'Stop sharing?', back: true, html: `<p>Disable every Live link for <strong>${esc(node.name)}</strong>. Saved copies stay available. If this document only uses Live links, MARS also deletes its remote copy.</p>`, buttons: [{ label: 'Stop sharing', value: 'stop', danger: true }] });
    if (answer.value !== 'stop') return;
    ui.busy('Stopping sharing…');
    const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope)) ?? { members: [], invitations: [] };
    const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? []).filter(link => link.document === node.connection!.document && !revoked.invitations.includes(link.id));
    for (const link of links) { await revoke(scope, { link }); if (await rotationRepository(scope).get()) throw new Error('revocation-pending'); }
    await finishSharing(scope, node.connection!.document, ui.signal);
}
/** The longest device name that a pairing request carries. */
const MAX_DEVICE_NAME = 64;
/**
 * Authenticated data for the encrypted device name of a pairing request. It binds the requested
 * grant to the link key, which the service never receives.
 */
const pairLabelContext = async (invitation: string, grant: SignedGrant['grant']) => ({ invitation, grant: await hash(jsonBytes(grant)) });
/** True when `owner` is a well-formed, self-signed owner grant for the owner key of a link. */
async function ownerMatchesLink(owner: unknown, ownerKey: string): Promise<boolean> {
    return validGrant(owner) && !owner.issuer && owner.grant.permission === 'owner' && owner.grant.publicKey === ownerKey
        && verify(owner.grant, owner.signature, ownerKey);
}
async function syncLinkPanel(scope: Scope, ui: SyncDialog) {
    if (!ui.open) return;
    if (scope.selection) { await ui.screen({ title: 'Connect a device', back: true, html: '<p>Open the original synced folder to connect another device. This recovery contains selected documents only.</p>' }); return; }
    scope.personalSync = true; await saveScope(scope);
    ui.busy('Creating Sync link…');
    await viewer.flush(); await driverFor(scope)?.upload();
    if (await driverFor(scope)?.options.replica.pending()) {
        await ui.screen({ title: 'Sync link', back: true, html: '<p>Your latest changes have not finished syncing. Go back and try again before connecting another device.</p>' }); return;
    }
    const fragment = newSyncSecret(), access = await syncAccess(fragment), secret = access.token, invitation = { v: 1 as const, id: access.id, collection: scope.collection, document: null, mode: 'pair' as const,
        permission: 'edit' as const, publicKey: scope.identity.publicKey, participant: scope.member.grant.participant, expires: Math.floor(Date.now() / 1000) + LIMITS.invitationSeconds };
    const policy: SignedInvitation = { invitation, signature: await sign(invitation, scope.identity) };
    const link: PairLink = { v: 1, id: invitation.id, secret, ownerKey: scope.owner.grant.publicKey, wrappingKey: encode(access.key) };
    await scoped(scope, 'invite', { policy, tokenHash: await tokenHash(secret), bootstrap: await sealSyncBootstrap(link, fragment) });
    const url = `${location.origin}${location.pathname}#sync=${fragment}`;
    const controller = new AbortController(), close = () => controller.abort();
    ui.signal.addEventListener('abort', close, { once: true });
    let pending: Record<string, unknown> | undefined, problem = '';
    const name = app.node(scope.localRoot)?.name ?? 'this document';
    const screen = ui.screen({ title: 'Sync link', back: true, html: `<p>Open this link on your other device to sync <strong>${esc(name)}</strong>. Confirm the matching code here to approve the connection.</p><p class="sync-fineprint">The link expires in 10 minutes. Keep this window open until your other device connects.</p><p class="sync-fineprint">The other device connects as you and can edit this ${scope.kind === 'folder' ? 'folder' : 'document'}. For other people, use <strong>Share → Copy link</strong> and choose Live to set their access.</p>`,
        field: { label: 'Sync link', value: url, readonly: true }, buttons: [{ label: 'Copy link', value: 'copy', primary: true,
            run: button => copyRich(url).then(() => copyFeedback(button)).catch(() => copyFeedback(button, 'Could not copy. Select the link and copy it manually.')) }] });
    const waiting = waitForPair('pair-pending', { collection: scope.collection, member: scope.member.grant.member, invitation: invitation.id }, scope.token, controller, (value) => !!value.candidate)
        .then((value) => { pending = value; if (!controller.signal.aborted) ui.resolve(value?.candidate ? 'candidate' : 'back'); })
        .catch((error) => { if (!controller.signal.aborted) { problem = syncError(error); ui.resolve('error'); } });
    try {
        const result = await screen; controller.abort(); await waiting;
        if (result.value === 'error' && ui.open) { await ui.screen({ title: 'Sync link', back: true, html: `<p role="alert">${esc(problem)}</p>` }); return; }
        if (result.value !== 'candidate' || !pending?.candidate || !ui.open) return;
        // The request arrives through the service, so it is untrusted until two checks pass. The grant
        // must be exactly what a Sync link permits. The device name must decrypt under the link key
        // with this grant as authenticated data: the service never sees that key, so it cannot swap
        // in a grant of its own. The code that both screens show is the final check by the user.
        const candidate = pending.candidate as SignedGrant;
        let label: { name: string };
        try {
            if (!validGrant(candidate) || candidate.issuer || candidate.grant.collection !== scope.collection || candidate.grant.permission !== 'edit'
                || candidate.grant.document !== null || candidate.grant.participant !== invitation.participant || candidate.grant.expires !== 0
                || !await verify(candidate.grant, candidate.signature, candidate.grant.publicKey)) throw new Error('invalid-pair-request');
            label = await openJson<{ name: string }>(pending.label as { nonce: string; ciphertext: string }, access.key, await pairLabelContext(invitation.id, candidate.grant));
            if (typeof label?.name !== 'string') throw new Error('invalid-pair-request');
        } catch {
            await ui.screen({ title: 'Sync link', back: true, html: '<p role="alert">The request from the other device is not valid. Reload MARS on both devices, then create a new Sync link.</p>' });
            return;
        }
        const phrase = await pairingPhrase(policy, candidate.grant);
        const answer = await ui.screen({ title: 'Check the code', back: true, html: `<p><strong>${esc(label.name)}</strong> is ready to connect.</p><p>Does your other device show this same code?</p>${confirmationCode(phrase)}`,
            buttons: [{ label: 'Codes match', value: 'yes', primary: true }] });
        if (answer.value !== 'yes') return;
        const rootEpoch = sessions.get(scope.localRoot)?.record.binding.epoch ?? 1;
        const signed = await signGrant(candidate.grant, scope.identity), keys = [await wrapKey({ collection: scope.collection, document: scope.root, epoch: rootEpoch, member: signed.grant.member }, decode(scope.key), scope.identity, candidate.grant.boxKey)];
        ui.busy('Connecting your device…');
        await scoped(scope, 'pair-approve', { invitation: invitation.id, signed, keys });
        scope.clientNames = { ...scope.clientNames, [signed.grant.member]: label.name }; await saveScope(scope);
    } finally { controller.abort(); ui.signal.removeEventListener('abort', close); }
}
export async function receiveLocation(address: string) {
    const params = new URLSearchParams(new URL(address).hash.slice(1));
    if (params.has('live')) return receiveLive(address, params.get('live')!);
    const encoded = params.get('sync') ?? params.get('invite'); // Accept links issued before #sync=.
    if (encoded === null) return false;
    await initialize(); history.replaceState(history.state, '', location.pathname);
    let receiving: SyncDialog | undefined;
    const leave = () => receiving?.close();
    try {
        let link: PairLink, incoming;
        if (isShortSyncLink(encoded)) {
            const access = await syncAccess(encoded); incoming = await request('invite-read', { invitation: access.id }, access.token);
            link = await openSyncBootstrap(incoming.bootstrap, encoded);
        } else {
            const decoded = await decodeState(encoded); if (!decoded) throw new Error('Invalid Sync link');
            link = JSON.parse(decoded); linkWrappingKey(link); incoming = await request('invite-read', { invitation: link.id }, link.secret);
        }
        if (!validInvitation(incoming.policy) || !await ownerMatchesLink(incoming.owner, link.ownerKey)
            || !await verify(incoming.policy.invitation, incoming.policy.signature, link.ownerKey)) throw new Error('Invalid Sync link');
        const answer = await modal({ title: 'Sync', html: '<p>Connect this device to your synced document or folder. Your other documents stay separate.</p><p>Give this device a name so you can recognize it later.</p>', field: { label: 'Name', value: 'My device' },
            buttons: [{ label: 'Cancel', value: '' }, { label: 'Connect', value: 'yes', primary: true }] });
        if (answer.value !== 'yes') return true;
        const who = await identity(), token = randomSecret(), g = { v: 1 as const, collection: incoming.policy.invitation.collection, member: newId(), participant: incoming.policy.invitation.participant,
            permission: 'edit' as const, document: null, publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(token), expires: 0 };
        const candidate = await signGrant(g, who);
        const encryptedName = await sealJson({ name: answer.text.trim().slice(0, MAX_DEVICE_NAME) || 'My device' }, linkWrappingKey(link), await pairLabelContext(link.id, g));
        await request('pair-request', { invitation: link.id, candidate, label: encryptedName }, link.secret);
        const phrase = await pairingPhrase(incoming.policy, candidate.grant), controller = new AbortController();
        const waiting = modal({ title: 'Sync', html: `<p>Check that your other device shows this same code:</p>${confirmationCode(phrase)}<p role="status">Confirm the code there to finish connecting.</p>`, buttons: [{ label: 'Cancel', value: '' }] });
        waiting.then(() => controller.abort());
        const result = await waitForPair('pair-result', { invitation: link.id }, link.secret, controller, (value) => !!value.approved) as { approved: boolean; signed: SignedGrant; owner: SignedGrant; keys: WrappedKey[]; root: { document: string; kind: 'doc' | 'folder' } } | undefined;
        if (!result?.approved || controller.signal.aborted) return true;
        document.querySelector<HTMLDialogElement>('#marsModal')?.close(); await waiting;
        const ui = receiving = new SyncDialog('syncDownloadDialog'), folder = result.root.kind === 'folder';
        void ui.screen({ title: folder ? 'Downloading synced folder' : 'Downloading synced document',
            html: `<p id="syncDownloadName">${folder ? 'Preparing your synced folder.' : 'Preparing your synced document.'}</p><progress class="sync-download-progress" aria-label="Sync download" aria-describedby="syncDownloadStatus"></progress><p id="syncDownloadStatus" role="status" aria-atomic="true">${folder ? 'Reading folder details…' : 'Downloading document…'}</p><p class="sync-fineprint">Keep this tab open. ${folder ? 'The folder will appear with its complete hierarchy when the download finishes.' : 'The document will appear when the download finishes.'}</p>`,
            buttons: [{ label: 'Cancel', value: 'cancel' }] }).then(() => ui.close());
        window.addEventListener('pagehide', leave, { once: true });
        const meter = ui.element.querySelector('progress')!, status = ui.element.querySelector('#syncDownloadStatus')!;
        const progress = (completed: number, total: number) => {
            if (!ui.open) return;
            if (total) { meter.max = total; meter.value = completed; }
            status.textContent = total ? `${completed} of ${total} ${total === 1 ? 'document' : 'documents'} downloaded${completed === total ? '. Saving folder…' : '…'}` : 'Saving folder hierarchy…';
        };
        // The approval also arrives through the service. Accept it only when the owner key from the
        // link signed exactly the grant that this device requested, and wrapped the root key itself.
        const wrapped = Array.isArray(result.keys) ? result.keys[0] : undefined;
        if (!await ownerMatchesLink(result.owner, link.ownerKey) || !validGrant(result.signed) || result.signed.issuer
            || canonical(result.signed.grant) !== canonical(g) || !await verifyGrant(result.signed, result.owner.grant)
            || !validWrappedKey(wrapped) || wrapped.sender !== result.owner.grant.boxKey || wrapped.collection !== g.collection || wrapped.member !== g.member
            || !validId(result.root?.document) || wrapped.document !== result.root.document || !['doc', 'folder'].includes(result.root.kind)) throw new Error('Invalid approval');
        const rootKey = await unwrapKey(result.keys[0], who), root = result.root;
        const local = newId(), node: TreeNode = { id: local, parentId: null, kind: root.kind, name: 'Sync', order: nextOrder(app.state.nodes, null), createdAt: Date.now(), updatedAt: Date.now() };
        const scope: Scope = { collection: g.collection, root: root.document, localRoot: local, kind: root.kind, token, key: encode(rootKey), owner: result.owner, member: result.signed, identity: who, exclusions: [] };
        const binding: SyncBinding = { collection: g.collection, document: root.document, member: g.member, permission: 'edit', kind: 'sync', epoch: result.keys[0].epoch, root: root.document,
            participant: scope.member.grant.participant, ownerParticipant: scope.owner.grant.participant };
        const record = Session.seed(binding, '', who, token, scope.key, result.owner, 'Sync'); record.member = result.signed;
        const empty = new BodyReplica(); record.body = encode(empty.checkpoint()); empty.destroy();
        const session = stagedSession(record);
        try {
            await loadPackets(scope, session, ui.signal, true); node.name = session.metadata().name ?? 'Sync';
            ui.element.querySelector('#syncDownloadName')!.textContent = node.name;
            node.connection = { ...session.record.binding, phase: 'working' };
            if (root.kind === 'folder') await materialize(scope, session, ui.signal, { bootstrap: node, progress });
            else {
                meter.max = meter.value = 1; status.textContent = 'Saving document…';
                const meta = session.metadata();
                Object.assign(node, { threads: openThreads(session.record.projection), size: textBytes(session.record.projection),
                    ...(validId(meta.origin) ? { originId: meta.origin } : {}),
                    ...(meta.readingWidth === 'full' || typeof meta.readingWidth === 'number' ? { readingWidth: meta.readingWidth } : {}) });
                await app.serializePersistence(async () => {
                    ui.signal.throwIfAborted();
                    await db.materializeConnected(node, session.record.body, [], { record: session.record, metadata: { [scopeId(scope)]: scope } });
                    await app.load();
                });
                scopes.set(scopeId(scope), scope);
                await initialize(true);
            }
        } finally { session.destroy(); }
        ui.close();
        await offerLocalMatches(scope); start(scope);
        if (root.kind === 'doc') await app.openDoc(scope.localRoot); else { app.state.folderId = scope.localRoot; await app.setOpen(scope.localRoot, true); }
        app.notify(); viewer.toast('Sync connected');
    } catch (error) { viewer.toast((error as Error)?.name === 'AbortError' ? 'Sync setup canceled.' : errorText(error) === 'Invalid Sync link' ? 'This Sync link is invalid. Get a new one from your other device.' : syncError(error)); }
    finally { window.removeEventListener('pagehide', leave); receiving?.close(); }
    return true;
}
async function offerLocalMatches(scope: Scope) {
    for (const incoming of app.state.nodes.filter((node) => node.kind === 'doc' && belongs(node, scope))) {
        const session = sessions.get(incoming.id); if (!session) continue;
        const remote = session.record.projection;
        let best: TreeNode | undefined, localText = '', score = 0;
        for (const local of app.state.nodes.filter((node) => node.kind === 'doc' && !node.connection)) {
            const text = await db.getText(local.id) ?? '';
            const match = local.originId && local.originId === incoming.originId ? 2 : matchScore({ name: incoming.name, text: remote }, { name: local.name, text });
            if (match > score) { best = local; localText = text; score = match; }
        }
        if (!best) continue;
        const merged = mergeDocuments(localText, remote);
        const answer = await modal({ title: 'Connect the existing copy?', html: `<p><strong>${esc(incoming.name)}</strong> matches <strong>${esc(best.name)}</strong> in this library.</p>
            <p>Names and content are matching hints, not Sync identities. Keep the copies separate, or connect the existing document.</p>
            <p>Connecting retains its local ID and moves it into the selected collection. Different local content stays in an independent backup.</p>`,
            buttons: [{ label: 'Keep copies separate', value: '', primary: true }, { label: 'Connect with my text', value: 'mine' }, ...(!merged.sameBody ? [{ label: 'Connect with Sync text', value: 'theirs' }] : [])] });
        if (!answer.value) continue;
        const next = answer.value === 'mine' ? merged.mine : merged.theirs;
        try {
            await validateSnapshot(session, next);
            await session.change(session.record.projection, next);
            const record = await session.repository.get(); if (!record) throw new Error('The incoming replica is unavailable');
            const connected = { ...best, parentId: incoming.parentId, name: incoming.name, connection: incoming.connection,
                originId: incoming.originId, localReadingWidth: best.localReadingWidth ?? best.readingWidth, manualFileSave: true,
                ...(app.bodyOf(localText) !== app.bodyOf(next) ? { pending: true as const } : {}),
                ...(best.handle && localText !== next ? { dirty: true } : {}) };
            const backup: TreeNode | undefined = localText !== next ? { id: newId(), originId: newId(), name: app.uniqueName(`${best.name} (before Sync)`, best.parentId),
                kind: 'doc', parentId: best.parentId, order: nextOrder(app.state.nodes, best.parentId), createdAt: Date.now(), updatedAt: Date.now(), readingWidth: best.readingWidth } : undefined;
            const updated = { ...scope, localRoot: scope.localRoot === incoming.id ? best.id : scope.localRoot };
            await db.adoptConnected(incoming.id, best, connected, localText, record, { [scopeId(scope)]: updated }, backup);
            session.destroy(); sessions.delete(incoming.id);
            Object.assign(scope, updated); await app.load();
            sessions.set(best.id, new Session(record, repository(best.id)));
        } catch (error) { viewer.toast(`The copies stay separate: ${errorText(error)}. No existing local document was overwritten.`); }
    }
}
export async function unlink(id: string) {
    await initialize(); const node = app.node(id), scope = node ? rootFor(node) : undefined; if (!node || !scope) return;
    await viewer.flush();
    const nodes = [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])].filter((n) => belongs(n, scope));
    const answer = await modal({ title: 'Stop sync?', body: 'Your document stays on this device as an independent copy. Other connected devices keep syncing.',
        buttons: [{ label: 'Keep syncing', value: '' }, { label: 'Stop sync', value: 'yes', danger: true }] });
    if (answer.value !== 'yes') return;
    await disconnect(scope, nodes);
    viewer.refreshConnection(); app.notify();
}
async function disconnect(scope: Scope, nodes: TreeNode[]) {
    driverFor(scope)?.stop();
    for (const item of nodes) if (!scope.exclusions.includes(item.connection!.document)) scope.exclusions.push(item.connection!.document);
    await saveScope(scope);
    await app.serializePersistence(async () => {
        for (const item of nodes) {
            sessions.get(item.id)?.destroy(); sessions.delete(item.id); await db.deleteSide('sync', item.id);
            delete item.connection; item.originId = newId(); if (item.handle) item.manualFileSave = true;
        }
        await db.putNodes(nodes);
    });
    if (!app.state.nodes.some((n) => belongs(n, scope))) { driverFor(scope)?.dispose(); drivers.delete(scopeId(scope)); }
    else driverFor(scope)?.retry();
}
export async function remove(id: string, remote = false) {
    await initialize(); await viewer.flush();
    if (viewer.hasConnectionDraftConflict()) throw new Error('Save the draft before deleting this document.');
    const node = app.node(id); if (!node) return;
    const scope = rootFor(node), all = [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])];
    if (remote) {
        if (!scope || scope.member.grant.permission !== 'owner') throw new Error('Only the owner can delete remote content.');
        const connected = all.filter((item) => belongs(item, scope));
        for (const item of all.filter((item) => !belongs(item, scope))) if (item.parentId && connected.some((parent) => parent.id === item.parentId)) await app.move(item.id, null);
        for (const item of connected) {
            try { await scoped(scope, 'delete-document', { document: item.connection!.document }); }
            catch (error) {
                // Expiry/deletion already ended remote access, including a lost response
                // to an earlier delete. Still let the reader remove the retained copy.
                if (!['connection-ended', 'connection-expired', 'document-deleted', 'document-expired'].includes(errorText(error))) throw error;
            }
        }
        await disconnect(scope, connected);
    } else {
        for (const scope of scopes.values()) {
            const connected = all.filter(item => belongs(item, scope));
            if (connected.length) await disconnect(scope, connected);
        }
    }
}
export async function deletionInfo(ids: string[]) {
    await initialize();
    const selected = app.state.nodes.filter(node => ids.includes(node.id)), now = Date.now() / 1000;
    let liveLinks = 0, synced = 0;
    for (const scope of scopes.values()) {
        const nodes = selected.filter(node => belongs(node, scope) && node.connection!.phase !== 'ended');
        if (!nodes.length) continue;
        if (!scope.live && (scope.personalSync !== false || scope.kind === 'folder')) synced += nodes.filter(node => node.kind === 'doc').length;
        const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope));
        const documents = new Set(nodes.map(node => node.connection!.document));
        const links = await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? [];
        liveLinks += links.filter(link => documents.has(link.document) && link.expires > now && !revoked?.invitations.includes(link.id)).length;
    }
    return { liveLinks, synced };
}
export async function beforeMove(id: string, parent: string | null) {
    await initialize(); const node = app.node(id), target = app.node(parent), scope = node ? rootFor(node) : undefined;
    if (!node) return false;
    if (scope?.kind === 'folder' && node.id !== scope.localRoot && target?.connection?.collection !== scope.collection) { await unlink(id); if (node.connection) return false; }
    if (!node.connection && target?.connection) {
        const answer = await modal({ title: 'Add to Sync?', body: 'Moving this local content into the synced folder uploads its text and comments after the move.', buttons: [{ label: 'Cancel', value: '' }, { label: 'Move and sync', value: 'yes', primary: true }] });
        if (answer.value !== 'yes') return false;
    }
    return true;
}
export async function undo(id: string, redo = false) { const session = sessions.get(id); if (!session) return; const text = await session.undo(redo); await persistProjection(id, text); void driverFor(rootFor(app.node(id)!))?.upload(); }
export async function markReviewed(id: string) { const session = sessions.get(id); if (!session) return; await session.markReviewed(); void driverFor(rootFor(app.node(id)!))?.upload(); }
export async function undoReview(id: string, before: import('../share/merge.ts').Thread[], after: import('../share/merge.ts').Thread[]) {
    const session = sessions.get(id); if (!session) return; const text = await session.undoReview(before, after);
    await persistProjection(id, text); void driverFor(rootFor(app.node(id)!))?.upload();
}
export async function sharedWidth(id: string, width: number | 'full') {
    const session = sessions.get(id); if (!session) return;
    await session.metadataChanged({ readingWidth: width }); const node = app.node(id)!; node.readingWidth = width;
    void driverFor(rootFor(node))?.upload();
}

export async function live(id: string) {
    return (await import('../share/share.ts')).copyLinkDialog(id, 'live');
}
export async function createLiveLink(id: string, permission: 'view' | 'comment' | 'edit', ttl: number, format: 'short' | 'full' = 'short') {
    await initialize(); const node = app.node(id); if (!node || node.kind !== 'doc') throw new Error('Choose a document to share live.');
    if (node.connection && rootFor(node)?.member.grant.permission !== 'owner') throw new Error('Create Live links from the original sharing device.');
    if (!node.connection) await enrollSelection(node.id, false);
    const scope = rootFor(node), session = sessions.get(node.id); if (!scope || !session) throw new Error('sync-not-ready');
    await viewer.flush(); await driverFor(scope)?.upload();
    if (await driverFor(scope)?.options.replica.pending()) throw new Error('sync-pending');
    const delegate = await identity(), fragment = newLiveSecret(), access = await liveAccess(fragment), secret = access.token, policy: SignedInvitation = {
        invitation: { v: 1, id: access.id, collection: scope.collection, document: node.connection!.document, mode: 'live', permission,
            publicKey: delegate.publicKey, participant: null, expires: Math.floor(Date.now() / 1000) + ttl }, signature: '' };
    policy.signature = await sign(policy.invitation, scope.identity);
    const link: LiveLink = { v: 1, id: policy.invitation.id, secret, ownerKey: scope.owner.grant.publicKey, signing: delegate.signing, key: session.record.key, epoch: session.record.binding.epoch, wrappingKey: encode(access.key) };
    await scoped(scope, 'invite', { policy, tokenHash: await tokenHash(secret), bootstrap: await sealLiveBootstrap(link, fragment) });
    const url = `${location.origin}${location.pathname}#live=${format === 'full' ? await encodeState(JSON.stringify(link)) : fragment}`;
    node.connection!.liveShared = true; await db.putNodes([node]);
    const links = await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? [];
    links.push({ id: policy.invitation.id, document: node.connection!.document, expires: policy.invitation.expires, secret, wrappingKey: link.wrappingKey, permission }); await db.setMeta(`sync:links:${scopeId(scope)}`, links);
    await writeLiveKey(scope, policy.invitation.id, link.wrappingKey, node.connection!.document, session.record.binding.epoch, session.record.key);
    return { url, expires: policy.invitation.expires };
}
async function receiveLive(address: string, encoded: string) {
    if (!db.isTemporaryStorage()) { const url = new URL(address); url.searchParams.set('temporary', '1'); location.assign(url); return true; }
    await initialize();
    try {
        let link: LiveLink, incoming;
        if (isShortLiveLink(encoded)) {
            const access = await liveAccess(encoded);
            incoming = await request('invite-read', { invitation: access.id }, access.token);
            link = await openLiveBootstrap(incoming.bootstrap, encoded);
        } else {
            const raw = await decodeState(encoded); if (!raw) throw new Error('Invalid live link');
            link = JSON.parse(raw); linkWrappingKey(link); incoming = await request('invite-read', { invitation: link.id }, link.secret);
        }
        if (!validInvitation(incoming.policy) || !await ownerMatchesLink(incoming.owner, link.ownerKey)
            || incoming.policy.invitation.mode !== 'live' || !await verify(incoming.policy.invitation, incoming.policy.signature, link.ownerKey)) throw new Error('Invalid live-link signature');
        const policy: SignedInvitation = incoming.policy, who = await identity(), token = randomSecret(), memberId = newId();
        const updated = incoming.keyUpdate ? await readLiveKey(incoming.keyUpdate, link, incoming.owner) : { epoch: link.epoch ?? 1, key: link.key };
        const grant = { v: 1 as const, collection: policy.invitation.collection, member: memberId, participant: memberId,
            permission: policy.invitation.permission, document: policy.invitation.document, publicKey: who.publicKey, boxKey: who.boxKey,
            tokenHash: await tokenHash(token), expires: policy.invitation.expires };
        const signed = await signGrant(grant, { signing: link.signing, box: {}, publicKey: policy.invitation.publicKey, boxKey: '' }); signed.issuer = policy;
        await request('live-join', { invitation: link.id, signed }, link.secret);
        const id = newId(), node: TreeNode = { id, parentId: null, kind: 'doc', name: 'Live document', order: 0, createdAt: Date.now(), updatedAt: Date.now() };
        app.state.nodes.push(node);
        const scope: Scope = { collection: grant.collection, root: grant.document!, localRoot: id, kind: 'doc', key: updated.key, token,
            owner: incoming.owner, member: signed, identity: who, exclusions: [], live: link, liveUrl: address };
        await saveScope(scope);
        const binding: SyncBinding = { collection: grant.collection, document: grant.document!, root: grant.document!, member: memberId,
            permission: grant.permission, kind: 'live', epoch: updated.epoch };
        const record = Session.seed(binding, '', who, token, updated.key, incoming.owner, node.name, signed), empty = new BodyReplica(); record.body = encode(empty.checkpoint()); empty.destroy();
        record.keys = { [String(link.epoch ?? 1)]: link.key, [String(updated.epoch)]: updated.key };
        const session = await createSession(node, record); await loadPackets(scope, session); node.name = session.metadata().name ?? node.name;
        start(scope); await app.openDoc(id); viewer.endLoading(); app.notify();
    } catch (error) { viewer.endLoading(); viewer.toast(`Could not open the live link: ${errorText(error) === 'unsupported-link-key-format' ? syncError(error) : errorText(error)}`); }
    return true;
}
export async function saveLive(id: string) {
    await viewer.flush(); const node = app.node(id), session = sessions.get(id), scope = node ? rootFor(node) : undefined;
    if (!node || !session || !scope || node.connection?.kind !== 'live') return;
    const ended = node.connection.phase === 'ended';
    const answer = await modal({ title: ended ? 'Save local copy to library' : 'Save live document to library', body: ended ? 'Keep this copy and any unsent changes on this device so you can reopen it later. The old Live link remains disabled. Other temporary documents are not saved.' : 'Keep this document and any unsent changes on this device so you can reopen it later. It stays connected while the Live link is active. Other temporary documents are not saved.',
        buttons: [{ label: 'Cancel', value: '' }, { label: 'Save to library', value: 'yes', primary: true }] });
    if (answer.value !== 'yes') return;
    const record = await db.getSide<SessionRecord>('sync', id); if (!record) return;
    try {
        await db.promoteConnected(node, record.projection, record, { [scopeId(scope)]: scope, lastDoc: id });
        await app.load(); await initialize(true); history.replaceState(history.state, '', `${location.pathname}#doc=${id}`); app.notify(); viewer.toast('Live document saved to library');
    } catch (error) { viewer.toast(`The live session is still temporary: ${errorText(error)}. Download pending work before closing.`); }
}

interface OwnedLink { id: string; document: string; expires: number; secret: string; wrappingKey: string; permission?: 'view' | 'comment' | 'edit'; label?: string }
async function writeLiveKey(scope: Scope, invitation: string, wrappingKey: string, document: string, epoch: number, key: string) {
    const encrypted = await sealJson({ epoch, key }, decode(wrappingKey), { invitation });
    const value = { document, epoch, ...encrypted };
    await scoped(scope, 'live-key-write', { invitation, value, signature: await sign(value, scope.identity) });
}
async function readLiveKey(update: { value: { document: string; epoch: number; nonce: string; ciphertext: string }; signature: string }, link: LiveLink, owner: SignedGrant) {
    if (!await verify(update.value, update.signature, owner.grant.publicKey)) throw new Error('Invalid key update signature');
    return openJson<{ epoch: number; key: string }>(update.value, linkWrappingKey(link), { invitation: link.id });
}
async function refreshKey(scope: Scope, session: Session, epoch: number, signal?: AbortSignal, staged = false) {
    const id = session.record.binding.document, response = await scoped(scope, 'key-read', { document: id }, signal);
    let key: string | undefined;
    for (const value of response.values) {
        if (value.wrapped.epoch === epoch && value.wrapped.document === id && value.wrapped.member === scope.member.grant.member
            && await verify(value.wrapped, value.signature, scope.owner.grant.publicKey)) key = encode(await unwrapKey(value.wrapped, scope.identity));
    }
    if (!key && scope.live) {
        const bootstrap = await scoped(scope, 'bootstrap', {}, signal);
        if (bootstrap.keyUpdate) { const updated = await readLiveKey(bootstrap.keyUpdate, scope.live, scope.owner); if (updated.epoch === epoch) key = updated.key; }
    }
    if (!key) throw new Error('A new document key is required. Local work is retained.');
    await session.rekey(epoch, key);
    if (staged) { if (id === scope.root) scope.key = key; return; }
    const node = app.state.nodes.find((n) => belongs(n, scope) && n.connection!.document === id); if (node) node.connection!.epoch = epoch;
    if (id === scope.root) { scope.key = key; await saveScope(scope); }
}
async function revoke(scope: Scope, target: { member?: string; link?: OwnedLink }) {
    await viewer.flush(); driverFor(scope)?.stop();
    try {
        if (await rotationRepository(scope).get()) { await continueRotation(scope); return; }
        const members = (await scoped(scope, 'members')).members as SignedGrant[];
        for (const member of members) if (member.grant.collection !== scope.collection || !await verifyGrant(member, scope.owner.grant)) throw new Error('Invalid member signature');
        const links = await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? [];
        const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope)) ?? { members: [], invitations: [] };
        const plan: RotationPlan = { v: 1, id: newId(), revision: 1, collection: scope.collection, member: scope.member.grant.member,
            target: target.member ? { member: target.member } : { invitation: target.link!.id }, documents: [] };
        const affected = [...sessions.values()].filter((session) => session.record.binding.collection === scope.collection
            && session.record.binding.member === scope.member.grant.member && (!target.link || session.record.binding.document === target.link.document));
        for (const session of affected) {
            const document = session.record.binding.document, epoch = (await scoped(scope, 'epoch', { document })).epoch + 1, key = encode(keyBytes());
            const values: RotationDocument['values'] = [], updates: RotationDocument['links'] = [];
            for (const member of members) if (member.grant.member !== target.member && (!member.grant.document || member.grant.document === document)
                && !revoked.members.includes(member.grant.member) && (!member.issuer || !revoked.invitations.includes(member.issuer.invitation.id))
                && (!member.grant.expires || member.grant.expires > Date.now() / 1000) && (!target.link || member.issuer?.invitation.id !== target.link.id)) {
                const wrapped = await wrapKey({ collection: scope.collection, document, epoch, member: member.grant.member }, decode(key), scope.identity, member.grant.boxKey);
                values.push({ wrapped, signature: await sign(wrapped, scope.identity) });
            }
            for (const link of links) if (link.document === document && link.id !== target.link?.id && !revoked.invitations.includes(link.id) && link.expires > Date.now() / 1000) {
                // Unsupported links receive no new document key. Their records stay
                // available so the owner can explicitly disable them.
                if (!validLinkWrappingKey(link)) continue;
                const value = { document, epoch, ...await sealJson({ epoch, key }, linkWrappingKey(link), { invitation: link.id }) };
                updates.push({ invitation: link.id, value, signature: await sign(value, scope.identity) });
            }
            plan.documents.push({ document, epoch, key, values, links: updates });
        }
        await rotationRepository(scope).commit(null, plan);
        await continueRotation(scope);
    } catch (error) { phase(scope, 'attention'); viewer.toast(`Revocation needs attention: ${errorText(error)}. Publication is paused and local work is retained.`); }
}
async function continueRotation(scope: Scope) {
    if (rotationTasks.has(scopeId(scope))) return rotationTasks.get(scopeId(scope));
    const run = async () => {
        driverFor(scope)?.stop();
        await resumeRotation(rotationRepository(scope), (action, values) => scoped(scope, action, values), async ({ document, epoch, key }) => {
            const session = [...sessions.values()].find((session) => session.record.binding.document === document && session.record.binding.member === scope.member.grant.member);
            if (!session) throw new Error('Restore the local replica before resuming key rotation');
            if (session.record.binding.epoch > epoch) throw new Error('A newer rotation already changed this document');
            await session.rekey(epoch, key, true);
            if (document === scope.root) { scope.key = key; await saveScope(scope); }
            if (scope.kind === 'folder' && document !== scope.root) {
                const root = sessions.get(scope.localRoot)!, old = root.manifest().find((value) => value.id === document);
                if (old) await root.editManifest([{ ...old, key, epoch }]);
            }
        }, async plan => validateRotationLinks(plan, await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`) ?? []));
        start(scope); driverFor(scope)?.retry(); await driverFor(scope)?.upload(); viewer.toast('Access revoked and document keys rotated');
    };
    const task = ('locks' in navigator ? navigator.locks.request(rotationKey(scope), run) : run()).catch((error) => {
        phase(scope, 'attention'); viewer.toast(`Revocation needs attention: ${errorText(error)}. Use Retry to resume the saved key updates.`);
    }).finally(() => rotationTasks.delete(scopeId(scope)));
    rotationTasks.set(scopeId(scope), task); return task;
}
async function recoverDraft(id: string, session: Session, envelope: import('./protocol.ts').SignedCheckpoint, author: SignedGrant) {
    const draft = viewer.connectionDraft(); if (!draft) return;
    const original = app.node(id); if (!original) return;
    const copy = await app.createDoc({ name: `${original.name} (recovered draft)`, text: draft.text, parentId: null, open: false });
    viewer.discardConnectionDraft();
    const text = await session.receive(envelope, author); await persistProjection(id, text);
    await app.openDoc(copy.id); viewer.toast('Draft saved as an independent local copy');
}
export async function exportRecovery(ids: string[], password: string) {
    await initialize(); await viewer.flush();
    const selected = new Set(ids), connected = new Map<string, Scope>();
    for (const node of app.state.nodes) if (selected.has(node.id)) { const scope = rootFor(node); if (scope) connected.set(scopeId(scope), scope); }
    const connections = [], projections: Record<string, string> = {};
    for (const scope of connected.values()) {
        const captured = await db.captureConnected(scope.collection, scope.member.grant.member, [scopeId(scope), `sync:links:${scopeId(scope)}`, rotationKey(scope), revokedAccessKey(scope)], ids);
        const nodes = captured.nodes.filter(node => selected.has(node.id)).map(({ id, parentId, kind, name, order, connection, readingWidth }) => ({ id, parentId, kind, name, order, connection, readingWidth }));
        const saved = captured.records as SessionRecord[], records = nodes.flatMap(node => {
            const record = saved.find(record => record.binding.document === node.connection!.document);
            if (record && node.kind === 'doc') projections[node.id] = record.projection;
            return record ? [{ local: node.id, record }] : [];
        });
        const snapshot = structuredClone(captured.metadata[scopeId(scope)] ?? scope) as Scope;
        const root = nodes.find(node => node.id === snapshot.localRoot);
        if (!root || snapshot.selection) {
            const first = nodes.find(node => node.kind === 'doc'), record = records.find(item => item.local === first?.id)?.record;
            if (!first || !record) throw new Error('The selected connection has no document to recover.');
            snapshot.root = first.connection!.document; snapshot.localRoot = first.id; snapshot.kind = 'doc'; snapshot.key = record.key;
            snapshot.selection = nodes.filter(node => node.kind === 'doc').map(node => node.connection!.document);
            snapshot.exclusions = captured.excludedDocuments;
        }
        const rotation = captured.metadata[rotationKey(scope)] as RotationPlan | undefined;
        if (rotation?.documents.some(document => !nodes.some(node => node.connection!.document === document.document))) throw new Error('Finish the pending connection change before exporting its recovery data, or download documents only.');
        connections.push({ scope: snapshot, nodes, records, links: ((captured.metadata[`sync:links:${scopeId(scope)}`] ?? []) as OwnedLink[]).filter(link => nodes.some(node => node.connection!.document === link.document)), rotation,
            revoked: captured.metadata[revokedAccessKey(scope)] ?? { members: [], invitations: [] } });
    }
    const local = [];
    for (const node of app.state.nodes) if (selected.has(node.id) && !node.connection) {
        const { id, parentId, kind, name, order, readingWidth } = node;
        local.push({ node: { id, parentId, kind, name, order, readingWidth }, text: kind === 'doc' ? await app.docText(id) : null });
    }
    // Include preserved repair state in the existing encrypted backup, never in
    // ordinary downloads or a partial selection of a folder's documents.
    const savedSetups: db.SavedUnfinishedSetup[] = [], seenBackups = new Set<string>();
    for (const node of app.state.nodes) if (selected.has(node.id) && (node.kind === 'folder' ? descendants(app.state.nodes, node.id).every(child => selected.has(child.id)) : rootFor(node)?.localRoot === node.id)) {
        const entries = await db.getMeta<db.UnfinishedSetupBackup[]>(db.unfinishedSetupIndex(node.id)) ?? [];
        for (const entry of entries) if (!seenBackups.has(entry.key)) {
            const saved = await db.getMeta<db.SavedUnfinishedSetup>(entry.key); if (!saved) throw new Error('Missing preserved recovery data');
            savedSetups.push(saved); seenBackups.add(entry.key);
        }
    }
    const value = { v: 3, connections, local, ...(savedSetups.length ? { savedSetups } : {}) };
    try { return { name: 'MARS connections/mars-encrypted-recovery.json', bytes: new TextEncoder().encode(JSON.stringify(await encryptRecovery(value, password))), projections }; }
    catch (error) {
        if (errorText(error) !== 'recovery-package-too-large') throw error;
        return { name: 'MARS connections/mars-encrypted-recovery.mars-recovery', bytes: new Uint8Array(await (await encryptRecoveryArchive(value, password)).arrayBuffer()), projections };
    }
}
export async function restoreRecovery(selectedFile?: File) {
    if (db.isTemporaryStorage()) { viewer.toast('Open your library before restoring a backup.'); return; }
    const input = document.createElement('input'); input.type = 'file'; input.accept = 'application/json,.mars-recovery,.zip'; input.hidden = true; document.body.append(input);
    const restore = async (selected?: File) => {
        let file = selected; if (!file) return;
        if (/\.zip$/i.test(file.name)) {
            if (file.size > MAX_RECOVERY_ARCHIVE_BYTES) { viewer.toast('This ZIP is too large to restore here. Extract its connection backup and restore that file.'); return; }
            try {
                const entries = unzipSync(new Uint8Array(await file.arrayBuffer()), { filter: entry => /^MARS connections\/mars-encrypted-recovery\.(json|mars-recovery)$/.test(entry.name) && entry.originalSize <= MAX_RECOVERY_ARCHIVE_BYTES });
                const files = Object.entries(entries); if (files.length !== 1) throw new Error('Invalid recovery archive');
                file = new File([files[0][1] as BlobPart], files[0][0]);
            } catch { viewer.toast('This ZIP has no readable connection backup. Import its documents normally instead.'); return; }
        }
        const archive = await isRecoveryArchive(file);
        if (file.size > (archive ? MAX_RECOVERY_ARCHIVE_BYTES : MAX_RECOVERY_FILE_BYTES)) { viewer.toast('This backup is too large. Your library has not changed.'); return; }
        const answer = await modal({ title: 'Restore backup', body: 'Restore documents and sync settings from this backup. Existing documents stay unchanged. Close MARS on the original device before continuing.',
            field: { label: 'Backup password', value: '', type: 'password' }, buttons: [{ label: 'Cancel', value: '' }, { label: 'Restore', value: 'yes', primary: true }] });
        if (answer.value !== 'yes') return;
        try {
            type Recovery = { scope: Scope; nodes: TreeNode[]; records: { local: string; record: SessionRecord }[]; links?: OwnedLink[]; rotation?: RotationPlan | null; revoked?: RevokedAccess };
            type RecoveryPackage = Recovery | { v: number; connections: Recovery[]; local: { node: TreeNode; text: string | null }[] };
            type PreviousBackups = { v: number; kind: 'mars-unfinished-sync-backups'; snapshots: db.SavedUnfinishedSetup[] };
            const raw = archive ? await decryptRecoveryArchive<RecoveryPackage | PreviousBackups>(file, answer.text) : await decryptRecovery<RecoveryPackage | PreviousBackups>(JSON.parse(await file.text()), answer.text);
            let decoded: RecoveryPackage;
            if ('kind' in raw && raw.kind === 'mars-unfinished-sync-backups') {
                if (raw.v !== 1 || !Array.isArray(raw.snapshots) || !raw.snapshots.length) throw new Error('Invalid previous setup backups');
                const packages = raw.snapshots.map(recoveryFromUnfinished);
                decoded = { v: 3, connections: packages.flatMap(value => value.connections), local: packages.flatMap(value => value.local) } as RecoveryPackage;
            } else decoded = raw as RecoveryPackage;
            if ('savedSetups' in decoded) {
                if (!Array.isArray(decoded.savedSetups) || !('connections' in decoded)) throw new Error('Invalid preserved recovery data');
                const packages = (decoded.savedSetups as db.SavedUnfinishedSetup[]).map(recoveryFromUnfinished);
                const connections = [...decoded.connections], local = [...decoded.local], seen = new Set(connections.map(value => scopeId(value.scope)));
                for (const saved of packages) {
                    local.push(...saved.local as typeof local);
                    for (const connection of saved.connections) {
                        if (!seen.has(scopeId(connection.scope))) { connections.push(connection); seen.add(scopeId(connection.scope)); continue; }
                        // The current backup already contains this connection.
                        // Older competing projections remain independent local work.
                        for (const node of connection.nodes) local.push({ node: { ...node, connection: undefined }, text: node.kind === 'doc' ? connection.records.find(item => item.local === node.id)!.record.projection : null });
                    }
                }
                decoded = { ...decoded, connections, local } as RecoveryPackage;
            }
            const values = 'connections' in decoded ? decoded.connections : [decoded], local = 'local' in decoded ? decoded.local : [];
            if (!Array.isArray(values) || !values.length || !Array.isArray(local)
                || values.some(value => !Array.isArray(value?.nodes) || value.nodes.length > LIMITS.documents || !Array.isArray(value.records))) throw new Error('Invalid recovery contents');
            const allOriginal = [...values.flatMap(value => value.nodes), ...local.map(value => value.node)];
            if (new Set(allOriginal.map(node => node.id)).size !== allOriginal.length) throw new Error('Duplicate recovery document');
            const ids = new Map(allOriginal.map(node => [node.id, newId()])), allNodes: TreeNode[] = [], allRecords: Record<string, SessionRecord> = {}, metadata: Record<string, unknown> = {}, scopesSeen = new Set<string>();
            for (const value of values) {
            const scope = value?.scope;
            if (scope?.live) linkWrappingKey(scope.live);
            if (!scope || !validId(scope.collection) || !validId(scope.root) || !validId(scope.localRoot) || !['doc', 'folder'].includes(scope.kind)
                || !validGrant(scope.owner) || scope.owner.grant.permission !== 'owner' || scope.owner.grant.collection !== scope.collection
                || !await verifyGrant(scope.owner, scope.owner.grant) || !validGrant(scope.member) || scope.member.grant.collection !== scope.collection
                || !await verifyGrant(scope.member, scope.owner.grant) || scope.identity?.publicKey !== scope.member.grant.publicKey
                || !await verify({ kind: 'mars-recovery-identity' }, await sign({ kind: 'mars-recovery-identity' }, scope.identity), scope.member.grant.publicKey)
                || await tokenHash(scope.token) !== scope.member.grant.tokenHash || decode(scope.key).length !== 32
                || !Array.isArray(scope.exclusions) || scope.exclusions.length > LIMITS.documents || !scope.exclusions.every(validId)
                || !Array.isArray(value.nodes) || !value.nodes.length || value.nodes.length > LIMITS.documents || !Array.isArray(value.records)
                || value.records.length > value.nodes.length || new Set(value.nodes.map((node) => node.id)).size !== value.nodes.length
                || new Set(value.records.map((item) => item.local)).size !== value.records.length) throw new Error('Invalid recovery contents');
            if (scopesSeen.has(scopeId(scope))) throw new Error('Duplicate recovery connection'); scopesSeen.add(scopeId(scope));
            if (scope.selection && (!Array.isArray(scope.selection) || scope.selection.length > LIMITS.documents || !scope.selection.every(validId))) throw new Error('Invalid recovery selection');
            const root = value.nodes.find((node) => node.id === scope.localRoot);
            if (!root || root.kind !== scope.kind || root.connection?.document !== scope.root) throw new Error('Invalid recovery root');
            if (value.rotation && (scope.member.grant.permission !== 'owner' || value.rotation.collection !== scope.collection || value.rotation.member !== scope.member.grant.member
                || value.rotation.v !== 1 || !validId(value.rotation.id) || !Number.isSafeInteger(value.rotation.revision) || !Array.isArray(value.rotation.documents)
                || value.rotation.documents.length > LIMITS.documents || value.rotation.documents.some((item) => !validId(item.document) || !Number.isSafeInteger(item.epoch)
                    || item.epoch < 1 || decode(item.key).length !== 32 || !value.nodes.some((node) => node.connection?.document === item.document)))) throw new Error('Invalid recovery rotation');
            if (value.rotation) {
                const target = value.rotation.target;
                if (!target || !validId('member' in target ? target.member : target.invitation)) throw new Error('Invalid recovery rotation target');
                for (const document of value.rotation.documents) {
                    if (!Array.isArray(document.values) || document.values.length > LIMITS.members + LIMITS.liveGrants || !Array.isArray(document.links)
                        || document.links.length > LIMITS.liveGrants) throw new Error('Invalid recovery key distribution');
                    for (const item of document.values) if (!validWrappedKey(item.wrapped) || item.wrapped.collection !== scope.collection
                        || item.wrapped.document !== document.document || item.wrapped.epoch !== document.epoch || !await verify(item.wrapped, item.signature, scope.owner.grant.publicKey)) throw new Error('Invalid recovered key signature');
                    for (const item of document.links) if (!validId(item.invitation) || item.value?.document !== document.document || item.value.epoch !== document.epoch
                        || !await verify(item.value, item.signature, scope.owner.grant.publicKey)) throw new Error('Invalid recovered link signature');
                }
            }
            const revoked = value.revoked ?? { members: [], invitations: [] };
            if (!Array.isArray(revoked.members) || !Array.isArray(revoked.invitations) || !revoked.members.every(validId) || !revoked.invitations.every(validId)) throw new Error('Invalid recovered revocation history');
            const links = value.links ?? [];
            if (!Array.isArray(links) || links.length > LIMITS.liveGrants || links.some((link) => !validId(link.id) || !validId(link.document)
                || !Number.isSafeInteger(link.expires) || !validLinkWrappingKey(link)
                // A recovery file is untrusted input. These two fields reach dialog markup.
                || (link.permission !== undefined && !['view', 'comment', 'edit'].includes(link.permission))
                || (link.label !== undefined && (typeof link.label !== 'string' || link.label.length > 200))
                || !value.nodes.some((node) => node.connection?.document === link.document))) throw new Error('Invalid recovered links');
            if (value.rotation) await validateRotationLinks(value.rotation, links);
            for (const node of value.nodes) {
                if (!validId(node.id) || !['doc', 'folder'].includes(node.kind) || typeof node.name !== 'string' || node.name.length > 256
                    || !Number.isSafeInteger(node.order) || !belongs(node, scope) || !validId(node.connection?.document)
                    || node.connection.permission !== scope.member.grant.permission || scope.member.grant.document !== null && node.connection.document !== scope.member.grant.document) throw new Error('Invalid recovery node');
                const parents = new Set([node.id]); let parent = value.nodes.find((item) => item.id === node.parentId);
                while (parent) { if (parent.kind !== 'folder' || parents.has(parent.id)) throw new Error('Invalid recovery placement'); parents.add(parent.id); parent = value.nodes.find((item) => item.id === parent!.parentId); }
                const saved = value.records.find((item) => item.local === node.id)?.record;
                if ((node.kind === 'doc' || node.id === scope.localRoot) && !saved) throw new Error('Missing recovery replica');
                if (!saved) continue;
                if (saved.v !== 1 || saved.kind !== 'connected' || !Number.isSafeInteger(saved.revision) || saved.revision < 1
                    || saved.binding.collection !== scope.collection || saved.binding.member !== scope.member.grant.member
                    || saved.binding.document !== node.connection!.document || saved.member.grant.publicKey !== scope.member.grant.publicKey
                    || typeof saved.projection !== 'string' || !Array.isArray(saved.outbox) || decode(saved.key).length !== 32) throw new Error('Invalid recovery replica');
                const replica = new BodyReplica(decode(saved.body)); replica.destroy();
            }
            if (app.state.nodes.some((node) => belongs(node, value.scope))) throw new Error('This client connection is already in the library. Unlink it before restoring another copy.');
            value.scope.localRoot = ids.get(value.scope.localRoot)!;
            const nodes: TreeNode[] = [], records: Record<string, SessionRecord> = {};
            for (const item of value.nodes) {
                const node: TreeNode = { id: ids.get(item.id)!, parentId: ids.get(item.parentId ?? '') ?? null, kind: item.kind, name: item.name,
                    order: item.order, connection: { ...item.connection!, phase: 'waiting' }, createdAt: Date.now(), updatedAt: Date.now(), readingWidth: item.readingWidth };
                nodes.push(node);
                const saved = value.records.find((record) => record.local === item.id);
                if (saved) records[node.id] = { ...saved.record, token: scope.token, identity: scope.identity, owner: scope.owner, member: scope.member };
            }
            allNodes.push(...nodes); Object.assign(allRecords, records);
            Object.assign(metadata, { [scopeId(scope)]: scope, [`sync:links:${scopeId(scope)}`]: links, [rotationKey(scope)]: value.rotation ?? null, [revokedAccessKey(scope)]: revoked });
            }
            for (const item of local) {
                const node = item.node;
                if (typeof node.id !== 'string' || !node.id || node.id.length > 256 || node.connection || !['doc', 'folder'].includes(node.kind) || typeof node.name !== 'string' || node.name.length > 256
                    || !Number.isSafeInteger(node.order) || (node.kind === 'doc' ? typeof item.text !== 'string' : item.text !== null)) throw new Error('Invalid local recovery document');
                allNodes.push({ id: ids.get(node.id)!, parentId: ids.get(node.parentId ?? '') ?? null, kind: node.kind, name: node.name, order: node.order, createdAt: Date.now(), updatedAt: Date.now(), readingWidth: node.readingWidth });
            }
            const placements = new Map(allNodes.map(node => [node.id, node]));
            for (const node of allNodes) { const parents = new Set([node.id]); let parent = placements.get(node.parentId ?? ''); while (parent) { if (parent.kind !== 'folder' || parents.has(parent.id)) throw new Error('Invalid recovery placement'); parents.add(parent.id); parent = placements.get(parent.parentId ?? ''); } }
            // Retain original credentials, pending ciphertext and replica variants
            // in the restored encrypted-backup path, not only their projections.
            if ('savedSetups' in decoded) for (const saved of decoded.savedSetups as db.SavedUnfinishedSetup[]) {
                const record = Object.values(saved.snapshot.records)[0] as SessionRecord;
                const scope = metadata[scopeKey(record.binding.collection, record.binding.member)] as Scope | undefined;
                if (!scope) throw new Error('Missing preserved recovery scope');
                const key = `sync:unfinished-setup:${newId()}`, index = db.unfinishedSetupIndex(scope.localRoot);
                metadata[key] = saved;
                metadata[index] = [...(metadata[index] as db.UnfinishedSetupBackup[] ?? []), { key, at: saved.at, records: Object.keys(saved.snapshot.records).length }];
            }
            await db.restoreConnected(allNodes, allRecords, metadata, Object.fromEntries(local.filter(item => item.node.kind === 'doc').map(item => [ids.get(item.node.id)!, item.text as string])));
            await app.load(); await initialize(true);
            viewer.toast('Backup restored. Your documents and unsent changes are saved here.');
        } catch { viewer.toast('Could not restore this backup. Check the file and password. Your library has not changed.'); }
    };
    input.addEventListener('change', () => { const file = input.files?.[0]; input.remove(); void restore(file); }, { once: true });
    input.addEventListener('cancel', () => input.remove(), { once: true });
    if (selectedFile) { input.remove(); await restore(selectedFile); } else input.click();
}
