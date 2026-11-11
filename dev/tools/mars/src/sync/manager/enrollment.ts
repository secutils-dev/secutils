import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { descendants, newId, type TreeNode } from '../../tree/model.ts';
import { identity, keyBytes, randomSecret, tokenHash, signGrant } from '../crypto.ts';
import { encode, LIMITS, validId, validGrant, type SyncBinding } from '../protocol.ts';
import { Session, type SessionRecord, type ManifestNode } from '../session.ts';
import type { RotationPlan } from '../rotation.ts';
import { syncDebug, safeError, type SetupStage } from '../debug.ts';
import {
    scopeKey,
    type Scope,
    scopeId,
    type OwnedLink,
    rotationKey,
    revokedAccessKey,
    type RevokedAccess,
    request,
    scoped,
    scopes,
    sessions,
    repository,
    driverFor,
    phase,
} from './state.ts';
import { initialize, start } from './engine.ts';

export class SyncSetupError extends Error {
    readonly code: string;
    readonly stage: SetupStage;
    constructor(stage: SetupStage, cause: unknown) {
        super('Sync setup did not finish', { cause });
        this.name = 'SyncSetupError';
        this.stage = stage;
        this.code = safeError(cause).code;
        syncDebug.record('setup.failed', { stage, ...safeError(cause) }, true);
    }
}
function unfinishedScope(snapshot: db.UnfinishedSetupSnapshot, record: SessionRecord) {
    return (snapshot.metadata[scopeKey(record.binding.collection, record.binding.member)] ??
        snapshot.metadata[`sync:scope:${record.binding.collection}`]) as Scope | undefined;
}
export function canRestartUnfinished(snapshot: db.UnfinishedSetupSnapshot, id: string) {
    const records = Object.values(snapshot.records) as SessionRecord[];
    return (
        snapshot.nodes[0]?.id === id &&
        snapshot.nodes[0].kind === 'folder' &&
        !snapshot.nodes.some((node) => node.connection) &&
        records.length > 0 &&
        records.every((record) => {
            if (
                record?.kind !== 'connected' ||
                !record.binding ||
                !validId(record.binding.collection) ||
                !validId(record.binding.member)
            )
                return false;
            const scope = unfinishedScope(snapshot, record);
            return (
                scope?.kind === 'folder' &&
                scope.localRoot === id &&
                scope.collection === record.binding.collection &&
                validGrant(scope.member) &&
                validGrant(scope.owner) &&
                scope.member.grant.member === record.binding.member &&
                scope.member.grant.document === null
            );
        })
    );
}
// Convert the preserved raw snapshot only on explicit backup restoration. Current
// text and the old CRDT/outbox become separate copies, so neither can replace the other.
export function recoveryFromUnfinished(saved: db.SavedUnfinishedSetup) {
    const snapshot = saved.snapshot;
    if (
        saved.v !== 1 ||
        saved.kind !== 'mars-unfinished-sync' ||
        !Array.isArray(snapshot?.nodes) ||
        !snapshot.records ||
        !snapshot.metadata ||
        !snapshot.texts
    )
        throw new Error('Invalid unfinished setup backup');
    const grouped = new Map<
        string,
        { scope: Scope; nodes: TreeNode[]; records: { local: string; record: SessionRecord }[] }
    >();
    const preservedProjections: { node: TreeNode; text: string }[] = [];
    const records = Object.entries(snapshot.records).sort(
        ([a, x], [b, y]) =>
            Number(b === unfinishedScope(snapshot, y as SessionRecord)?.localRoot) -
            Number(a === unfinishedScope(snapshot, x as SessionRecord)?.localRoot),
    );
    for (const [id, value] of records) {
        const record = value as SessionRecord,
            node = snapshot.nodes.find((node) => node.id === id);
        if (!node || record?.kind !== 'connected' || !record.binding)
            throw new Error('Invalid unfinished setup replica');
        const scope = unfinishedScope(snapshot, record);
        if (!scope) throw new Error('Missing unfinished setup credentials');
        let group = grouped.get(scopeId(scope));
        if (!group) {
            group = { scope: structuredClone(scope), nodes: [], records: [] };
            grouped.set(scopeId(scope), group);
        }
        // Repair archives can include the old competing replicas. Restore one
        // active binding, and retain every other projection as a local copy.
        const duplicate = group.nodes.find((item) => item.connection!.document === record.binding.document);
        if (duplicate) {
            preservedProjections.push({
                node: {
                    ...node,
                    id: newId(),
                    parentId: null,
                    connection: undefined,
                    name: `${node.name} (preserved Sync work)`,
                },
                text: record.projection,
            });
            continue;
        }
        const { parentId, kind, name, order, readingWidth } = node;
        group.nodes.push({
            id,
            parentId,
            kind,
            name,
            order,
            readingWidth,
            createdAt: saved.at,
            updatedAt: saved.at,
            connection: { ...record.binding, phase: 'waiting' },
        });
        group.records.push({ local: id, record });
    }
    const connections = [...grouped.values()].map((group) => {
        const originalScope = structuredClone(group.scope),
            root = group.nodes.find((node) => node.id === group.scope.localRoot);
        if (!root || group.scope.selection) {
            const first = group.nodes.find((node) => node.kind === 'doc'),
                record = group.records.find((item) => item.local === first?.id)?.record;
            if (!first || !record) throw new Error('Missing unfinished setup document');
            group.scope.root = first.connection!.document;
            group.scope.localRoot = first.id;
            group.scope.kind = 'doc';
            group.scope.key = record.key;
            group.scope.selection = group.nodes
                .filter((node) => node.kind === 'doc')
                .map((node) => node.connection!.document);
        }
        const ids = new Set(group.nodes.map((node) => node.id));
        for (const node of group.nodes)
            node.parentId =
                node.id === group.scope.localRoot
                    ? null
                    : ids.has(node.parentId ?? '')
                      ? node.parentId
                      : group.scope.kind === 'folder'
                        ? group.scope.localRoot
                        : null;
        const documents = new Set(group.nodes.map((node) => node.connection!.document));
        const links = (snapshot.metadata[`sync:links:${scopeId(originalScope)}`] ?? []) as OwnedLink[];
        return {
            ...group,
            links: links.filter((link) => documents.has(link.document)),
            rotation: snapshot.metadata[rotationKey(originalScope)] as RotationPlan | null | undefined,
            revoked: snapshot.metadata[revokedAccessKey(originalScope)] as RevokedAccess | undefined,
        };
    });
    const ids = new Map(snapshot.nodes.map((node) => [node.id, newId()]));
    const local = [
        ...snapshot.nodes.map((node) => ({
            node: {
                id: ids.get(node.id)!,
                parentId: ids.get(node.parentId ?? '') ?? null,
                kind: node.kind,
                name: node.name,
                order: node.order,
                readingWidth: node.readingWidth,
            },
            text: node.kind === 'doc' ? snapshot.texts[node.id] : null,
        })),
        ...preservedProjections,
    ];
    for (const group of connections) {
        const ids = new Map(group.nodes.map((node) => [node.id, newId()]));
        group.scope.localRoot = ids.get(group.scope.localRoot)!;
        group.nodes = group.nodes.map((node) => ({
            ...node,
            id: ids.get(node.id)!,
            parentId: ids.get(node.parentId ?? '') ?? null,
        }));
        group.records = group.records.map((item) => ({ ...item, local: ids.get(item.local)! }));
    }
    return { v: 3, connections, local };
}
export async function enrollSelection(id: string, personalSync = true, previous?: db.UnfinishedSetupSnapshot) {
    let stage: SetupStage = 'initializing',
        startedScope: Scope | undefined;
    const advance = (next: SetupStage, values: Record<string, unknown> = {}) => {
        stage = next;
        syncDebug.record('setup.stage', { stage, ...values });
    };
    try {
        advance('initializing', {
            temporary: db.isTemporaryStorage(),
            online: navigator.onLine,
            hidden: document.hidden,
        });
        await initialize();
        const node = app.node(id);
        if (!node) throw new db.ReplicaConflict('missing-node');
        if (node.connection) throw new db.ReplicaConflict('already-connected');
        if (db.isTemporaryStorage()) throw new Error('temporary-storage');
        advance('saving-edits');
        await viewer.flush();
        advance('checking-selection');
        const selected = structuredClone(
            previous?.nodes ?? [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])],
        );
        const stored = new Map((await db.allNodes()).map((item) => [item.id, item]));
        const storedConnected = selected.filter((item) => stored.get(item.id)?.connection).length;
        const orphanRecords = (
            await Promise.all(
                selected.filter((item) => !stored.get(item.id)?.connection).map((item) => db.getSide('sync', item.id)),
            )
        ).filter(Boolean).length;
        syncDebug.record('setup.stage', {
            stage,
            items: selected.length,
            documents: selected.filter((item) => item.kind === 'doc').length,
            folders: selected.filter((item) => item.kind === 'folder').length,
            connected: selected.filter((item) => item.connection).length,
            storedConnected,
            orphanRecords,
        });
        if (selected.some((n) => n.connection) || storedConnected) throw new db.ReplicaConflict('already-connected');
        if (selected.length > LIMITS.documents) throw new Error('selection-too-large');
        if (orphanRecords && !previous) throw new db.ReplicaConflict('orphan-replica');
        if (previous && !canRestartUnfinished(previous, id)) throw new Error('unfinished-setup-unavailable');
        const who = await identity(),
            token = randomSecret(),
            root = newId(),
            key = encode(keyBytes()),
            collection = newId();
        const grant = {
            v: 1 as const,
            collection,
            member: newId(),
            participant: newId(),
            permission: 'owner' as const,
            document: null,
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(token),
            expires: 0,
        };
        const owner = await signGrant(grant, who),
            scope: Scope = {
                collection,
                root,
                localRoot: id,
                kind: node.kind,
                key,
                token,
                owner,
                member: owner,
                identity: who,
                exclusions: [],
                personalSync,
                lastRenewed: Date.now(),
            };
        startedScope = scope;
        const ids = new Map(selected.map((n) => [n.id, n.id === id ? root : newId()]));
        advance('reading-documents');
        const texts: Record<string, string> = previous ? { ...previous.texts } : {};
        for (const item of selected)
            if (item.kind === 'doc') texts[item.id] = previous ? previous.texts[item.id] : await app.docText(item.id);
        advance('creating-collection');
        await request('create', { owner, root: { document: root, kind: node.kind } }, token);
        const values: ManifestNode[] = [],
            records: Record<string, SessionRecord> = {},
            connected = structuredClone(selected);
        for (const [index, item] of selected.entries()) {
            advance('registering-items', { item: index + 1, items: selected.length });
            const remote = ids.get(item.id)!,
                docKey = item.id === id ? key : encode(keyBytes());
            await scoped(scope, 'document', { document: remote, kind: item.kind });
            const binding: SyncBinding = {
                collection,
                document: remote,
                member: grant.member,
                permission: 'owner',
                kind: 'sync',
                epoch: 1,
                root,
            };
            binding.participant = grant.participant;
            binding.ownerParticipant = grant.participant;
            const target = connected.find((node) => node.id === item.id)!;
            target.connection = { ...binding, phase: 'waiting' };
            if (target.handle) target.manualFileSave = true;
            if (item.kind === 'doc' || item.id === id) {
                let record = Session.seed(binding, texts[item.id] ?? '', who, token, docKey, owner, item.name);
                const session = new Session(record, {
                    get: async () => structuredClone(record),
                    commit: async (_expected, next) => {
                        record = structuredClone(next);
                    },
                });
                try {
                    await session.metadataChanged({
                        kind: item.kind,
                        origin: validId(item.originId) ? item.originId : item.id,
                        ...(item.readingWidth !== undefined ? { readingWidth: item.readingWidth } : {}),
                    });
                    await session.initial();
                    records[item.id] = record;
                } finally {
                    session.destroy();
                }
            }
            values.push({
                id: remote,
                kind: item.kind,
                parent: item.id === id ? null : (ids.get(item.parentId!) ?? root),
                name: item.name,
                order: item.order,
                ...(item.kind === 'doc'
                    ? {
                          key: docKey,
                          origin: item.originId && !/^(file|url):/.test(item.originId) ? item.originId : item.id,
                      }
                    : {}),
            });
        }
        advance('preparing-manifest');
        if (scope.kind === 'folder') {
            const session = new Session(records[id], {
                get: async () => structuredClone(records[id]),
                commit: async (_expected, next) => {
                    records[id] = structuredClone(next);
                },
            });
            try {
                await session.editManifest(values);
            } finally {
                session.destroy();
            }
        }
        advance('saving-connections');
        await app.serializePersistence(async () => {
            await db.enrollConnected(selected, texts, connected, records, { [scopeId(scope)]: scope }, previous);
            scopes.set(scopeId(scope), scope);
            for (const item of connected) {
                const current = app.node(item.id);
                if (current) {
                    current.connection = item.connection;
                    if (item.manualFileSave) current.manualFileSave = true;
                }
                if (records[item.id]) {
                    sessions.get(item.id)?.destroy();
                    sessions.set(item.id, new Session(records[item.id], repository(item.id)));
                }
            }
        });
        advance('uploading');
        start(scope);
        const driver = driverFor(scope)!;
        await driver.upload();
        if (await driver.options.replica.pending()) throw driver.lastFailure ?? new Error('sync-pending');
        viewer.refreshConnection();
        app.notify();
        syncDebug.record('setup.finished', { items: selected.length });
    } catch (error) {
        if (startedScope && scopes.has(scopeId(startedScope))) phase(startedScope, 'attention');
        throw new SyncSetupError(stage, error);
    }
}
