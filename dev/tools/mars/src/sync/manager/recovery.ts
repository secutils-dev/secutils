// Encrypted connection backups: export of selected connections and local documents, and their restore.
// A backup is the only export that carries credentials, document keys, pending uploads, owned link
// secrets, revocation history, and an unfinished key rotation. The file formats and their encryption
// live in `../crypto.ts` (version 1 JSON) and `../recovery-archive.ts` (chunked archive).
// A recovery file is untrusted input, including after a successful decryption. `restoreRecovery` checks
// size, authentication, structure, signatures, and key material before one write transaction.
// A failed check writes nothing. Restored nodes get fresh local ids and cannot replace an existing node.
import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { descendants, newId, type TreeNode } from '../../tree/model.ts';
import { modal } from '../../ui/ui.ts';
import {
    tokenHash,
    sign,
    verify,
    verifyGrant,
    encryptRecovery,
    decryptRecovery,
    MAX_RECOVERY_FILE_BYTES,
} from '../crypto.ts';
import { decode, LIMITS, validId, validGrant, validWrappedKey } from '../protocol.ts';
import type { SessionRecord } from '../session.ts';
import { BodyReplica } from '../replica.ts';
import { validateRotationLinks, type RotationPlan } from '../rotation.ts';
import {
    encryptRecoveryArchive,
    decryptRecoveryArchive,
    isRecoveryArchive,
    MAX_RECOVERY_ARCHIVE_BYTES,
} from '../recovery-archive.ts';
import { linkWrappingKey, validLinkWrappingKey } from '../live-link.ts';
import { unzipSync } from 'fflate';
import {
    type Scope,
    rootFor,
    scopeId,
    rotationKey,
    revokedAccessKey,
    type OwnedLink,
    errorText,
    type RevokedAccess,
    belongs,
    scopeKey,
} from './state.ts';
import { initialize } from './engine.ts';
import { recoveryFromUnfinished } from './enrollment.ts';

/**
 * Builds the encrypted backup for the selected nodes `ids`.
 *
 * Each connection that the selection touches is captured in one read transaction: nodes, session records,
 * scope credentials, owned links, the rotation plan, and the revocation history.
 * A selection without the connection root becomes a document-scoped package. It lists the selected
 * documents in `selection` and the unselected documents in `exclusions`.
 * Selected local documents travel with their text. Preserved setup snapshots travel only with a completely
 * selected folder, or with the root document of a connection.
 *
 * @param password At least 12 characters.
 * @returns The ZIP entry name, the file bytes, and `projections`: the Markdown text of each connected
 * document from the same snapshot, by node id. A package above the version 1 size limit uses the archive format.
 * @throws Error when a connection has no document in the selection, when a pending rotation covers an
 * unselected document, or when preserved data is missing. The encryption step throws `SyncError`
 * `recovery-password-too-short` or `recovery-package-too-large`.
 */
export async function exportRecovery(ids: string[], password: string) {
    await initialize();
    await viewer.flush();
    const selected = new Set(ids),
        connected = new Map<string, Scope>();
    for (const node of app.state.nodes)
        if (selected.has(node.id)) {
            const scope = rootFor(node);
            if (scope) connected.set(scopeId(scope), scope);
        }
    const connections = [],
        projections: Record<string, string> = {};
    for (const scope of connected.values()) {
        const captured = await db.captureConnected(
            scope.collection,
            scope.member.grant.member,
            [scopeId(scope), `sync:links:${scopeId(scope)}`, rotationKey(scope), revokedAccessKey(scope)],
            ids,
        );
        // Only these node fields enter the backup. A file handle and other local state stay out.
        const nodes = captured.nodes
            .filter((node) => selected.has(node.id))
            .map(({ id, parentId, kind, name, order, connection, readingWidth }) => ({
                id,
                parentId,
                kind,
                name,
                order,
                connection,
                readingWidth,
            }));
        const saved = captured.records as SessionRecord[],
            records = nodes.flatMap((node) => {
                const record = saved.find((record) => record.binding.document === node.connection!.document);
                if (record && node.kind === 'doc') projections[node.id] = record.projection;
                return record ? [{ local: node.id, record }] : [];
            });
        const snapshot = structuredClone(captured.metadata[scopeId(scope)] ?? scope) as Scope;
        const root = nodes.find((node) => node.id === snapshot.localRoot);
        // Partial selection: re-root the package at the first selected document.
        // A restore then covers the selected documents and does not enroll the rest of the folder.
        if (!root || snapshot.selection) {
            const first = nodes.find((node) => node.kind === 'doc'),
                record = records.find((item) => item.local === first?.id)?.record;
            if (!first || !record) throw new Error('The selected connection has no document to recover.');
            snapshot.root = first.connection!.document;
            snapshot.localRoot = first.id;
            snapshot.kind = 'doc';
            snapshot.key = record.key;
            snapshot.selection = nodes.filter((node) => node.kind === 'doc').map((node) => node.connection!.document);
            snapshot.exclusions = captured.excludedDocuments;
        }
        const rotation = captured.metadata[rotationKey(scope)] as RotationPlan | undefined;
        // A pending rotation needs the session of each planned document. A package without one cannot resume it.
        if (
            rotation?.documents.some(
                (document) => !nodes.some((node) => node.connection!.document === document.document),
            )
        )
            throw new Error(
                'Finish the pending connection change before exporting its recovery data, or download documents only.',
            );
        connections.push({
            scope: snapshot,
            nodes,
            records,
            links: ((captured.metadata[`sync:links:${scopeId(scope)}`] ?? []) as OwnedLink[]).filter((link) =>
                nodes.some((node) => node.connection!.document === link.document),
            ),
            rotation,
            revoked: captured.metadata[revokedAccessKey(scope)] ?? { members: [], invitations: [] },
        });
    }
    const local = [];
    for (const node of app.state.nodes)
        if (selected.has(node.id) && !node.connection) {
            const { id, parentId, kind, name, order, readingWidth } = node;
            local.push({
                node: { id, parentId, kind, name, order, readingWidth },
                text: kind === 'doc' ? await app.docText(id) : null,
            });
        }
    // Include preserved repair state in the existing encrypted backup, never in
    // ordinary downloads or a partial selection of a folder's documents.
    const savedSetups: db.SavedUnfinishedSetup[] = [],
        seenBackups = new Set<string>();
    for (const node of app.state.nodes)
        if (
            selected.has(node.id) &&
            (node.kind === 'folder'
                ? descendants(app.state.nodes, node.id).every((child) => selected.has(child.id))
                : rootFor(node)?.localRoot === node.id)
        ) {
            const entries = (await db.getMeta<db.UnfinishedSetupBackup[]>(db.unfinishedSetupIndex(node.id))) ?? [];
            for (const entry of entries)
                if (!seenBackups.has(entry.key)) {
                    const saved = await db.getMeta<db.SavedUnfinishedSetup>(entry.key);
                    if (!saved) throw new Error('Missing preserved recovery data');
                    savedSetups.push(saved);
                    seenBackups.add(entry.key);
                }
        }
    const value = { v: 3, connections, local, ...(savedSetups.length ? { savedSetups } : {}) };
    // The version 1 JSON file has a 16 MiB limit. A larger package uses the chunked archive format.
    try {
        return {
            name: 'MARS connections/mars-encrypted-recovery.json',
            bytes: new TextEncoder().encode(JSON.stringify(await encryptRecovery(value, password))),
            projections,
        };
    } catch (error) {
        if (errorText(error) !== 'recovery-package-too-large') throw error;
        return {
            name: 'MARS connections/mars-encrypted-recovery.mars-recovery',
            bytes: new Uint8Array(await (await encryptRecoveryArchive(value, password)).arrayBuffer()),
            projections,
        };
    }
}
/**
 * Restores a backup into the library. Without `selectedFile` the function opens a file picker.
 *
 * Accepted input: a version 1 JSON package, a `.mars-recovery` archive, or a ZIP that holds exactly one
 * of them under `MARS connections/`. The reader enters the backup password in a dialog.
 *
 * The file is untrusted input. Size limits apply first, before the content is read or decrypted.
 * Decryption authenticates the content: a changed, reordered, or truncated file fails there.
 * The decrypted package must then pass each check below before the single write:
 * - structure and bounds of each connection, node, session record, and local document;
 * - signatures: a self-signed owner grant, and a member grant that verifies against it;
 * - a signing key and a token that match the member grant;
 * - the owner signature on each pending key packet;
 * - key material: 32-byte document keys, and for each link a wrapping key that differs from its token;
 * - the library holds no node of the same client connection.
 *
 * Then `db.restoreConnected` writes all nodes, records, and metadata in one transaction. It rejects a
 * node id or a client connection that exists.
 *
 * Each restored node gets a fresh local id, and each connected node starts in the `waiting` phase.
 * A failure after the password prompt shows one generic toast, and the library stays unchanged.
 * A temporary session cannot restore a backup.
 */
export async function restoreRecovery(selectedFile?: File) {
    if (db.isTemporaryStorage()) {
        viewer.toast('Open your library before restoring a backup.');
        return;
    }
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'application/json,.mars-recovery,.zip';
    input.hidden = true;
    document.body.append(input);
    const restore = async (selected?: File) => {
        let file = selected;
        if (!file) return;
        // A download ZIP holds the backup beside the documents. Extract only the backup entry, and skip
        // an entry whose declared size exceeds the archive limit.
        if (/\.zip$/i.test(file.name)) {
            if (file.size > MAX_RECOVERY_ARCHIVE_BYTES) {
                viewer.toast(
                    'This ZIP is too large to restore here. Extract its connection backup and restore that file.',
                );
                return;
            }
            try {
                const entries = unzipSync(new Uint8Array(await file.arrayBuffer()), {
                    filter: (entry) =>
                        /^MARS connections\/mars-encrypted-recovery\.(json|mars-recovery)$/.test(entry.name) &&
                        entry.originalSize <= MAX_RECOVERY_ARCHIVE_BYTES,
                });
                const files = Object.entries(entries);
                if (files.length !== 1) throw new Error('Invalid recovery archive');
                file = new File([files[0][1] as BlobPart], files[0][0]);
            } catch {
                viewer.toast('This ZIP has no readable connection backup. Import its documents normally instead.');
                return;
            }
        }
        // Bound the input before the password prompt and before any decryption. The marker selects the format.
        const archive = await isRecoveryArchive(file);
        if (file.size > (archive ? MAX_RECOVERY_ARCHIVE_BYTES : MAX_RECOVERY_FILE_BYTES)) {
            viewer.toast('This backup is too large. Your library has not changed.');
            return;
        }
        const answer = await modal({
            title: 'Restore backup',
            body: 'Restore documents and sync settings from this backup. Existing documents stay unchanged. Close MARS on the original device before continuing.',
            field: { label: 'Backup password', value: '', type: 'password' },
            buttons: [
                { label: 'Cancel', value: '' },
                { label: 'Restore', value: 'yes', primary: true },
            ],
        });
        if (answer.value !== 'yes') return;
        try {
            // One connection in the shape that `exportRecovery` writes.
            type Recovery = {
                scope: Scope;
                nodes: TreeNode[];
                records: { local: string; record: SessionRecord }[];
                links?: OwnedLink[];
                rotation?: RotationPlan | null;
                revoked?: RevokedAccess;
            };
            // A legacy single connection, or the version 3 group of connections and local documents.
            type RecoveryPackage =
                Recovery | { v: number; connections: Recovery[]; local: { node: TreeNode; text: string | null }[] };
            // A payload that holds preserved setup snapshots and no package.
            type PreviousBackups = {
                v: number;
                kind: 'mars-unfinished-sync-backups';
                snapshots: db.SavedUnfinishedSetup[];
            };
            // AES-GCM authenticates the whole payload. The type parameters describe the expected shape only:
            // the decrypted value stays untrusted until the checks below pass.
            const raw = archive
                ? await decryptRecoveryArchive<RecoveryPackage | PreviousBackups>(file, answer.text)
                : await decryptRecovery<RecoveryPackage | PreviousBackups>(JSON.parse(await file.text()), answer.text);
            let decoded: RecoveryPackage;
            // A `mars-unfinished-sync-backups` payload holds setup snapshots only. Convert it to the
            // version 3 package shape.
            if ('kind' in raw && raw.kind === 'mars-unfinished-sync-backups') {
                if (raw.v !== 1 || !Array.isArray(raw.snapshots) || !raw.snapshots.length)
                    throw new Error('Invalid previous setup backups');
                const packages = raw.snapshots.map(recoveryFromUnfinished);
                decoded = {
                    v: 3,
                    connections: packages.flatMap((value) => value.connections),
                    local: packages.flatMap((value) => value.local),
                } as RecoveryPackage;
            } else decoded = raw as RecoveryPackage;
            // Preserved setup snapshots add their connections and local copies to the package.
            // They pass through the same checks as the other connections.
            if ('savedSetups' in decoded) {
                if (!Array.isArray(decoded.savedSetups) || !('connections' in decoded))
                    throw new Error('Invalid preserved recovery data');
                const packages = (decoded.savedSetups as db.SavedUnfinishedSetup[]).map(recoveryFromUnfinished);
                const connections = [...decoded.connections],
                    local = [...decoded.local],
                    seen = new Set(connections.map((value) => scopeId(value.scope)));
                for (const saved of packages) {
                    local.push(...(saved.local as typeof local));
                    for (const connection of saved.connections) {
                        if (!seen.has(scopeId(connection.scope))) {
                            connections.push(connection);
                            seen.add(scopeId(connection.scope));
                            continue;
                        }
                        // The current backup already contains this connection.
                        // Older competing projections remain independent local work.
                        for (const node of connection.nodes)
                            local.push({
                                node: { ...node, connection: undefined },
                                text:
                                    node.kind === 'doc'
                                        ? connection.records.find((item) => item.local === node.id)!.record.projection
                                        : null,
                            });
                    }
                }
                decoded = { ...decoded, connections, local } as RecoveryPackage;
            }
            // A legacy payload is one connection without the `connections` wrapper.
            const values = 'connections' in decoded ? decoded.connections : [decoded],
                local = 'local' in decoded ? decoded.local : [];
            // Shape and bounds first. The later checks index into these arrays.
            if (
                !Array.isArray(values) ||
                !values.length ||
                !Array.isArray(local) ||
                values.some(
                    (value) =>
                        !Array.isArray(value?.nodes) ||
                        value.nodes.length > LIMITS.documents ||
                        !Array.isArray(value.records),
                )
            )
                throw new Error('Invalid recovery contents');
            const allOriginal = [...values.flatMap((value) => value.nodes), ...local.map((value) => value.node)];
            if (new Set(allOriginal.map((node) => node.id)).size !== allOriginal.length)
                throw new Error('Duplicate recovery document');
            // Each id from the file maps to a fresh local id. No restored node can take the id of a library node.
            const ids = new Map(allOriginal.map((node) => [node.id, newId()])),
                allNodes: TreeNode[] = [],
                allRecords: Record<string, SessionRecord> = {},
                metadata: Record<string, unknown> = {},
                scopesSeen = new Set<string>();
            for (const value of values) {
                const scope = value?.scope;
                // A received Live connection needs a wrapping key that differs from its bearer token.
                if (scope?.live) linkWrappingKey(scope.live);
                // The credentials must agree with each other, so a crafted file cannot mix foreign keys:
                // - the owner grant is self-signed, and the member grant verifies against it;
                // - the private signing key produces a signature that the member public key accepts;
                // - the token hashes to the value in the member grant;
                // - the scope key decodes to 32 bytes.
                if (
                    !scope ||
                    !validId(scope.collection) ||
                    !validId(scope.root) ||
                    !validId(scope.localRoot) ||
                    !['doc', 'folder'].includes(scope.kind) ||
                    !validGrant(scope.owner) ||
                    scope.owner.grant.permission !== 'owner' ||
                    scope.owner.grant.collection !== scope.collection ||
                    !(await verifyGrant(scope.owner, scope.owner.grant)) ||
                    !validGrant(scope.member) ||
                    scope.member.grant.collection !== scope.collection ||
                    !(await verifyGrant(scope.member, scope.owner.grant)) ||
                    scope.identity?.publicKey !== scope.member.grant.publicKey ||
                    !(await verify(
                        { kind: 'mars-recovery-identity' },
                        await sign({ kind: 'mars-recovery-identity' }, scope.identity),
                        scope.member.grant.publicKey,
                    )) ||
                    (await tokenHash(scope.token)) !== scope.member.grant.tokenHash ||
                    decode(scope.key).length !== 32 ||
                    !Array.isArray(scope.exclusions) ||
                    scope.exclusions.length > LIMITS.documents ||
                    !scope.exclusions.every(validId) ||
                    !Array.isArray(value.nodes) ||
                    !value.nodes.length ||
                    value.nodes.length > LIMITS.documents ||
                    !Array.isArray(value.records) ||
                    value.records.length > value.nodes.length ||
                    new Set(value.nodes.map((node) => node.id)).size !== value.nodes.length ||
                    new Set(value.records.map((item) => item.local)).size !== value.records.length
                )
                    throw new Error('Invalid recovery contents');
                if (scopesSeen.has(scopeId(scope))) throw new Error('Duplicate recovery connection');
                scopesSeen.add(scopeId(scope));
                if (
                    scope.selection &&
                    (!Array.isArray(scope.selection) ||
                        scope.selection.length > LIMITS.documents ||
                        !scope.selection.every(validId))
                )
                    throw new Error('Invalid recovery selection');
                const root = value.nodes.find((node) => node.id === scope.localRoot);
                if (!root || root.kind !== scope.kind || root.connection?.document !== scope.root)
                    throw new Error('Invalid recovery root');
                // Only the owner holds a rotation plan. It must name this collection and member, and each
                // planned document must be in the package.
                if (
                    value.rotation &&
                    (scope.member.grant.permission !== 'owner' ||
                        value.rotation.collection !== scope.collection ||
                        value.rotation.member !== scope.member.grant.member ||
                        value.rotation.v !== 1 ||
                        !validId(value.rotation.id) ||
                        !Number.isSafeInteger(value.rotation.revision) ||
                        !Array.isArray(value.rotation.documents) ||
                        value.rotation.documents.length > LIMITS.documents ||
                        value.rotation.documents.some(
                            (item) =>
                                !validId(item.document) ||
                                !Number.isSafeInteger(item.epoch) ||
                                item.epoch < 1 ||
                                decode(item.key).length !== 32 ||
                                !value.nodes.some((node) => node.connection?.document === item.document),
                        ))
                )
                    throw new Error('Invalid recovery rotation');
                if (value.rotation) {
                    const target = value.rotation.target;
                    if (!target || !validId('member' in target ? target.member : target.invitation))
                        throw new Error('Invalid recovery rotation target');
                    // A resumed plan uploads its saved packets unchanged. Each packet must therefore name
                    // its document and epoch and carry the owner signature.
                    for (const document of value.rotation.documents) {
                        if (
                            !Array.isArray(document.values) ||
                            document.values.length > LIMITS.members + LIMITS.liveGrants ||
                            !Array.isArray(document.links) ||
                            document.links.length > LIMITS.liveGrants
                        )
                            throw new Error('Invalid recovery key distribution');
                        for (const item of document.values)
                            if (
                                !validWrappedKey(item.wrapped) ||
                                item.wrapped.collection !== scope.collection ||
                                item.wrapped.document !== document.document ||
                                item.wrapped.epoch !== document.epoch ||
                                !(await verify(item.wrapped, item.signature, scope.owner.grant.publicKey))
                            )
                                throw new Error('Invalid recovered key signature');
                        for (const item of document.links)
                            if (
                                !validId(item.invitation) ||
                                item.value?.document !== document.document ||
                                item.value.epoch !== document.epoch ||
                                !(await verify(item.value, item.signature, scope.owner.grant.publicKey))
                            )
                                throw new Error('Invalid recovered link signature');
                    }
                }
                const revoked = value.revoked ?? { members: [], invitations: [] };
                if (
                    !Array.isArray(revoked.members) ||
                    !Array.isArray(revoked.invitations) ||
                    !revoked.members.every(validId) ||
                    !revoked.invitations.every(validId)
                )
                    throw new Error('Invalid recovered revocation history');
                // Owned links: a bounded count, valid ids, a supported key format, and a document in the package.
                const links = value.links ?? [];
                if (
                    !Array.isArray(links) ||
                    links.length > LIMITS.liveGrants ||
                    links.some(
                        (link) =>
                            !validId(link.id) ||
                            !validId(link.document) ||
                            !Number.isSafeInteger(link.expires) ||
                            !validLinkWrappingKey(link) ||
                            // A recovery file is untrusted input. These two fields reach dialog markup.
                            (link.permission !== undefined && !['view', 'comment', 'edit'].includes(link.permission)) ||
                            (link.label !== undefined && (typeof link.label !== 'string' || link.label.length > 200)) ||
                            !value.nodes.some((node) => node.connection?.document === link.document),
                    )
                )
                    throw new Error('Invalid recovered links');
                // Pending link packets must decrypt with the wrapping keys of these links and hold the planned key.
                if (value.rotation) await validateRotationLinks(value.rotation, links);
                for (const node of value.nodes) {
                    // Each node must belong to this connection and carry the permission of the member grant.
                    // A document-scoped grant admits only its own document.
                    if (
                        !validId(node.id) ||
                        !['doc', 'folder'].includes(node.kind) ||
                        typeof node.name !== 'string' ||
                        node.name.length > 256 ||
                        !Number.isSafeInteger(node.order) ||
                        !belongs(node, scope) ||
                        !validId(node.connection?.document) ||
                        node.connection.permission !== scope.member.grant.permission ||
                        (scope.member.grant.document !== null &&
                            node.connection.document !== scope.member.grant.document)
                    )
                        throw new Error('Invalid recovery node');
                    // Placement inside the connection: each ancestor is a folder, and the chain has no cycle.
                    const parents = new Set([node.id]);
                    let parent = value.nodes.find((item) => item.id === node.parentId);
                    while (parent) {
                        if (parent.kind !== 'folder' || parents.has(parent.id))
                            throw new Error('Invalid recovery placement');
                        parents.add(parent.id);
                        parent = value.nodes.find((item) => item.id === parent!.parentId);
                    }
                    const saved = value.records.find((item) => item.local === node.id)?.record;
                    if ((node.kind === 'doc' || node.id === scope.localRoot) && !saved)
                        throw new Error('Missing recovery replica');
                    if (!saved) continue;
                    // The session record must bind to this collection, this member, and the document of its node.
                    if (
                        saved.v !== 1 ||
                        saved.kind !== 'connected' ||
                        !Number.isSafeInteger(saved.revision) ||
                        saved.revision < 1 ||
                        saved.binding.collection !== scope.collection ||
                        saved.binding.member !== scope.member.grant.member ||
                        saved.binding.document !== node.connection!.document ||
                        saved.member.grant.publicKey !== scope.member.grant.publicKey ||
                        typeof saved.projection !== 'string' ||
                        !Array.isArray(saved.outbox) ||
                        decode(saved.key).length !== 32
                    )
                        throw new Error('Invalid recovery replica');
                    // The body must decode and load as a replica update. A failure here rejects the package.
                    const replica = new BodyReplica(decode(saved.body));
                    replica.destroy();
                }
                // A library holds one copy of a client connection. Scope metadata is keyed by collection and
                // member, so a second copy would replace the saved state of the first.
                if (app.state.nodes.some((node) => belongs(node, value.scope)))
                    throw new Error(
                        'This client connection is already in the library. Unlink it before restoring another copy.',
                    );
                value.scope.localRoot = ids.get(value.scope.localRoot)!;
                const nodes: TreeNode[] = [],
                    records: Record<string, SessionRecord> = {};
                // Rebuild each node from an allowlist of fields. A parent outside the package becomes the top level.
                for (const item of value.nodes) {
                    const node: TreeNode = {
                        id: ids.get(item.id)!,
                        parentId: ids.get(item.parentId ?? '') ?? null,
                        kind: item.kind,
                        name: item.name,
                        order: item.order,
                        connection: { ...item.connection!, phase: 'waiting' },
                        createdAt: Date.now(),
                        updatedAt: Date.now(),
                        readingWidth: item.readingWidth,
                    };
                    nodes.push(node);
                    const saved = value.records.find((record) => record.local === item.id);
                    // The verified scope credentials replace the copies inside each session record.
                    if (saved)
                        records[node.id] = {
                            ...saved.record,
                            token: scope.token,
                            identity: scope.identity,
                            owner: scope.owner,
                            member: scope.member,
                        };
                }
                allNodes.push(...nodes);
                Object.assign(allRecords, records);
                Object.assign(metadata, {
                    [scopeId(scope)]: scope,
                    [`sync:links:${scopeId(scope)}`]: links,
                    [rotationKey(scope)]: value.rotation ?? null,
                    [revokedAccessKey(scope)]: revoked,
                });
            }
            // Local documents carry no connection. A document has text, and a folder has none.
            for (const item of local) {
                const node = item.node;
                if (
                    typeof node.id !== 'string' ||
                    !node.id ||
                    node.id.length > 256 ||
                    node.connection ||
                    !['doc', 'folder'].includes(node.kind) ||
                    typeof node.name !== 'string' ||
                    node.name.length > 256 ||
                    !Number.isSafeInteger(node.order) ||
                    (node.kind === 'doc' ? typeof item.text !== 'string' : item.text !== null)
                )
                    throw new Error('Invalid local recovery document');
                allNodes.push({
                    id: ids.get(node.id)!,
                    parentId: ids.get(node.parentId ?? '') ?? null,
                    kind: node.kind,
                    name: node.name,
                    order: node.order,
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                    readingWidth: node.readingWidth,
                });
            }
            // Check placement again across the whole package: a local document can sit under a connected folder.
            const placements = new Map(allNodes.map((node) => [node.id, node]));
            for (const node of allNodes) {
                const parents = new Set([node.id]);
                let parent = placements.get(node.parentId ?? '');
                while (parent) {
                    if (parent.kind !== 'folder' || parents.has(parent.id))
                        throw new Error('Invalid recovery placement');
                    parents.add(parent.id);
                    parent = placements.get(parent.parentId ?? '');
                }
            }
            // Retain original credentials, pending ciphertext and replica variants
            // in the restored encrypted-backup path, not only their projections.
            if ('savedSetups' in decoded)
                for (const saved of decoded.savedSetups as db.SavedUnfinishedSetup[]) {
                    const record = Object.values(saved.snapshot.records)[0] as SessionRecord;
                    const scope = metadata[scopeKey(record.binding.collection, record.binding.member)] as
                        Scope | undefined;
                    if (!scope) throw new Error('Missing preserved recovery scope');
                    const key = `sync:unfinished-setup:${newId()}`,
                        index = db.unfinishedSetupIndex(scope.localRoot);
                    metadata[key] = saved;
                    metadata[index] = [
                        ...((metadata[index] as db.UnfinishedSetupBackup[]) ?? []),
                        { key, at: saved.at, records: Object.keys(saved.snapshot.records).length },
                    ];
                }
            // The only write. One transaction adds every node, record, and text and stores the metadata.
            // It rejects an existing node id or client connection.
            await db.restoreConnected(
                allNodes,
                allRecords,
                metadata,
                Object.fromEntries(
                    local
                        .filter((item) => item.node.kind === 'doc')
                        .map((item) => [ids.get(item.node.id)!, item.text as string]),
                ),
            );
            await app.load();
            await initialize(true);
            viewer.toast('Backup restored. Your documents and unsent changes are saved here.');
        } catch {
            viewer.toast('Could not restore this backup. Check the file and password. Your library has not changed.');
        }
    };
    input.addEventListener(
        'change',
        () => {
            const file = input.files?.[0];
            input.remove();
            void restore(file);
        },
        { once: true },
    );
    input.addEventListener('cancel', () => input.remove(), { once: true });
    // A supplied file needs no picker.
    if (selectedFile) {
        input.remove();
        await restore(selectedFile);
    } else input.click();
}
