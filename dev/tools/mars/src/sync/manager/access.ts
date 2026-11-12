// Ends access: local disconnection, owner revocation with key rotation, deletion, and tree moves across
// a synced folder boundary.
// `disconnect` is local only. It never contacts the service and never revokes a grant.
// `revoke` is the owner path. It saves the complete key-rotation plan before the first remote change, and
// `continueRotation` resumes that plan after a reload or a lost response (`../rotation.ts`).
// Invariants: one rotation runs per scope, across tabs where Web Locks exist. A new document key is wrapped
// only for members whose grants verify against the owner grant, and sealed only for owned Live links.
import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { descendants, newId, type TreeNode } from '../../tree/model.ts';
import { esc, modal } from '../../ui/ui.ts';
import { keyBytes, sign, wrapKey, verifyGrant, sealJson } from '../crypto.ts';
import { encode, decode, validGrant, type SignedGrant } from '../protocol.ts';
import { resumeRotation, validateRotationLinks, type RotationPlan, type RotationDocument } from '../rotation.ts';
import { SyncDialog } from '../dialog.ts';
import { icon } from '../../ui/icons.ts';
import { linkWrappingKey, validLinkWrappingKey } from '../live-link.ts';
import {
    rootFor,
    belongs,
    type Scope,
    scoped,
    type RevokedAccess,
    revokedAccessKey,
    driverFor,
    saveScope,
    sessions,
    drivers,
    scopeId,
    errorText,
    scopes,
    type OwnedLink,
    rotationRepository,
    phase,
    rotationTasks,
    rotationKey,
} from './state.ts';
import { initialize, start } from './engine.ts';
import { personalSync } from './deletion.ts';
import { deletionAllowed } from '../deletion.ts';

/**
 * Detaches node `id`, and its descendants in the same scope, from the connection without a confirmation.
 * The local copies stay, and no request goes to the service. When node `id` is the open document, it reopens.
 *
 * @throws Error when the editor holds a draft that conflicts with a remote change.
 */
export async function stopHere(id: string) {
    const node = app.node(id),
        scope = node ? rootFor(node) : undefined;
    if (!node || !scope) return;
    await viewer.flush();
    if (viewer.hasConnectionDraftConflict()) throw new Error('Save the draft before stopping sync');
    const nodes = [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])].filter((item) =>
        belongs(item, scope),
    );
    await disconnect(scope, nodes);
    viewer.refreshConnection();
    app.notify();
    if (node.id === app.state.currentId) await app.openDoc(node.id);
}
/**
 * Runs the owner's **Sync devices** screens inside `ui` until the reader goes back or closes the dialog.
 *
 * The list shows the other devices: grants for the complete collection that have no invitation issuer
 * and no local revocation record. Live-link recipients do not appear. A confirmed choice revokes that
 * device through `revoke`, which verifies each grant signature before it distributes keys.
 * A failed member request rejects.
 */
export async function accessPanel(scope: Scope, ui: SyncDialog) {
    while (ui.open) {
        // The member list is a service response. Keep only well-formed grants: their ids reach HTML attributes.
        const listed = (await scoped(scope, 'members')).members;
        const members = (Array.isArray(listed) ? listed : []).filter(validGrant);
        const revoked = (await db.getMeta<RevokedAccess>(revokedAccessKey(scope))) ?? { members: [], invitations: [] };
        const devices = members.filter(
            (member) =>
                member.grant.member !== scope.member.grant.member &&
                member.grant.document === null &&
                !member.issuer &&
                !revoked.members.includes(member.grant.member),
        );
        const title = app.node(scope.localRoot)?.name ?? 'this document';
        // A device name comes from the pairing request of that device, or from a restored backup.
        // It reaches the markup only through `esc()`.
        const deviceRows = devices
            .map((member, index) => {
                const name = scope.clientNames?.[member.grant.member] ?? `Device ${index + 1}`;
                return `<div class="sync-access-item"><span>${esc(name)}</span><button type="button" class="icon-btn icon-btn-s" data-tooltip="Disconnect device" aria-label="Disconnect ${esc(name)}" data-sync-action="device:${member.grant.member}">${icon('unlink')}</button></div>`;
            })
            .join('');
        const answer = await ui.screen({
            title: 'Sync devices',
            back: true,
            html: `<p>Devices syncing <strong>${esc(title)}</strong>.</p><div class="sync-access-list">${deviceRows || '<p>No other devices connected yet. Use a Sync link to connect one.</p>'}</div>`,
        });
        if (answer.value === 'back' || answer.value === 'close') return;
        const id = answer.value.split(':')[1],
            member = devices.find((member) => member.grant.member === id);
        if (!member) continue;
        const label = scope.clientNames?.[member.grant.member] ?? 'this device';
        const confirm = await ui.screen({
            title: 'Disconnect this device?',
            back: true,
            html: `<p><strong>${esc(label)}</strong> will stop syncing. Local copies and unsent changes stay on that device.</p>`,
            buttons: [{ label: 'Disconnect device', value: 'remove', primary: true }],
        });
        if (confirm.value === 'close') return;
        if (confirm.value === 'remove') {
            ui.busy('Disconnecting device…');
            await revoke(scope, { member: id });
        }
    }
}
/**
 * Asks **Stop sync?**, then detaches node `id` and its descendants in the same scope.
 * The local copies stay as independent documents. Other devices keep their grants and continue to sync.
 */
export async function unlink(id: string) {
    await initialize();
    const node = app.node(id),
        scope = node ? rootFor(node) : undefined;
    if (!node || !scope) return;
    await viewer.flush();
    const nodes = [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])].filter((n) =>
        belongs(n, scope),
    );
    const answer = await modal({
        title: 'Stop sync?',
        body: 'Your document stays on this device as an independent copy. Other connected devices keep syncing.',
        buttons: [
            { label: 'Keep syncing', value: '' },
            { label: 'Stop sync', value: 'yes', danger: true },
        ],
    });
    if (answer.value !== 'yes') return;
    await disconnect(scope, nodes);
    viewer.refreshConnection();
    app.notify();
}
/**
 * Detaches `nodes` from `scope` on this client only. It sends no request and revokes nothing.
 *
 * For each node it destroys the session, deletes the `sync` side record (replica, outbox, keys), removes
 * the connection, and assigns a fresh origin id. The stored text stays. A linked file switches to manual save.
 * The transport is disposed when no node of the scope remains. Otherwise it restarts.
 *
 * @param nodes Connected nodes of `scope`. Each one must have a `connection`.
 */
export async function disconnect(scope: Scope, nodes: TreeNode[]) {
    driverFor(scope)?.stop();
    // The exclusion list stops the engine from downloading or materializing these documents again.
    for (const item of nodes)
        if (!scope.exclusions.includes(item.connection!.document)) scope.exclusions.push(item.connection!.document);
    await saveScope(scope);
    // Run inside the persistence queue, so a pending document save cannot write between these steps.
    await app.serializePersistence(async () => {
        for (const item of nodes) {
            sessions.get(item.id)?.destroy();
            sessions.delete(item.id);
            await db.deleteSide('sync', item.id);
            delete item.connection;
            item.originId = newId();
            if (item.handle) item.manualFileSave = true;
        }
        await db.putNodes(nodes);
    });
    if (!app.state.nodes.some((n) => belongs(n, scope))) {
        driverFor(scope)?.dispose();
        drivers.delete(scopeId(scope));
    } else driverFor(scope)?.retry();
}
/**
 * Detaches node `id` and its subtree from their connections before the caller deletes the local nodes.
 * This function does not delete a local node.
 *
 * With `remote`, the owner also deletes each connected document from the collection. Local-only children
 * of a connected folder move to the top level first. A lost response is safe to retry: the service codes
 * for an ended or deleted document count as success.
 * Without `remote`, the nodes detach locally in every scope that the subtree touches.
 *
 * @throws Error when the editor holds a conflicting draft, when `remote` is set without the owner grant,
 * or when a deletion request fails. Documents that the service already deleted stay deleted.
 */
export async function remove(id: string, remote = false) {
    await initialize();
    await viewer.flush();
    if (viewer.hasConnectionDraftConflict()) throw new Error('Save the draft before deleting this document.');
    const node = app.node(id);
    if (!node) return;
    const scope = rootFor(node),
        all = [node, ...(node.kind === 'folder' ? descendants(app.state.nodes, id) : [])];
    if (remote) {
        if (!scope || scope.member.grant.permission !== 'owner')
            throw new Error('Only the owner can delete remote content.');
        const connected = all.filter((item) => belongs(item, scope));
        // Items outside this scope are not part of the collection. Keep them: move each one that sits
        // directly inside a connected folder to the top level, together with its own subtree.
        for (const item of all.filter((item) => !belongs(item, scope)))
            if (item.parentId && connected.some((parent) => parent.id === item.parentId)) await app.move(item.id, null);
        for (const item of connected) {
            try {
                await scoped(scope, 'delete-document', { document: item.connection!.document });
            } catch (error) {
                // Expiry/deletion already ended remote access, including a lost response
                // to an earlier delete. Still let the reader remove the retained copy.
                if (
                    !['connection-ended', 'connection-expired', 'document-deleted', 'document-expired'].includes(
                        errorText(error),
                    )
                )
                    throw error;
            }
        }
        await disconnect(scope, connected);
    } else {
        for (const scope of scopes.values()) {
            const connected = all.filter((item) => belongs(item, scope));
            if (connected.length) await disconnect(scope, connected);
        }
    }
}
/**
 * Counts what a deletion of the nodes `ids` affects, for the confirmation dialog. The counts come from local state.
 *
 * @returns `synced`: selected documents in a personal Sync scope with an active connection.
 * `liveLinks`: owned Live links for the selected documents that are unexpired and not revoked.
 */
export async function deletionInfo(ids: string[]) {
    await initialize();
    const selected = app.state.nodes.filter((node) => ids.includes(node.id)),
        now = Date.now() / 1000;
    let liveLinks = 0,
        synced = 0;
    for (const scope of scopes.values()) {
        const nodes = selected.filter((node) => belongs(node, scope) && node.connection!.phase !== 'ended');
        if (!nodes.length) continue;
        // A received Live document and an owner's Live-only document do not count as personal Sync.
        if (!scope.live && (scope.personalSync !== false || scope.kind === 'folder'))
            synced += nodes.filter((node) => node.kind === 'doc').length;
        const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope));
        const documents = new Set(nodes.map((node) => node.connection!.document));
        const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? [];
        liveLinks += links.filter(
            (link) => documents.has(link.document) && link.expires > now && !revoked?.invitations.includes(link.id),
        ).length;
    }
    const root = selected.find((node) => node.id === ids[0]);
    const scope = root ? rootFor(root) : undefined;
    const shared = !!(
        root?.connection &&
        root.connection.phase !== 'ended' &&
        scope &&
        personalSync(scope) &&
        deletionAllowed(scope.member, scope.owner)
    );
    return { liveLinks, synced, shared };
}
/**
 * Decides whether the tree can move node `id` under `parent` (`null` is the top level). `app.move` calls
 * it when either side has a connection.
 *
 * A member of a synced folder that leaves its collection must stop syncing first, after a confirmation.
 * Local content that enters a connected folder needs a confirmation, because the move uploads it.
 *
 * @returns `false` when the reader declines, or when node `id` does not exist. The caller then cancels the move.
 */
export async function beforeMove(id: string, parent: string | null) {
    await initialize();
    const node = app.node(id),
        target = app.node(parent),
        scope = node ? rootFor(node) : undefined;
    if (!node) return false;
    if (
        scope?.kind === 'folder' &&
        node.id !== scope.localRoot &&
        target?.connection?.collection !== scope.collection
    ) {
        await unlink(id);
        if (node.connection) return false;
    }
    if (!node.connection && target?.connection) {
        const answer = await modal({
            title: 'Add to Sync?',
            body: 'Moving this local content into the synced folder uploads its text and comments after the move.',
            buttons: [
                { label: 'Cancel', value: '' },
                { label: 'Move and sync', value: 'yes', primary: true },
            ],
        });
        if (answer.value !== 'yes') return false;
    }
    return true;
}
/**
 * Revokes one device (`target.member`, a member id) or one Live link (`target.link`) as the owner, and
 * rotates the document keys that the target could read.
 *
 * A device revocation rotates every local session of the scope. A link revocation rotates only the
 * document of that link. Each affected document gets a random 32-byte key at the service epoch plus one.
 * The key is wrapped for each remaining member and sealed for each remaining owned link.
 * The complete plan is saved before `continueRotation` sends the first change.
 *
 * A saved plan takes priority: the call resumes that plan and does not revoke `target`.
 * A failure after the editor flush does not reject. It sets the scope to `attention` and shows a toast,
 * and a saved plan stays for a retry.
 */
export async function revoke(scope: Scope, target: { member?: string; link?: OwnedLink }) {
    await viewer.flush();
    driverFor(scope)?.stop();
    try {
        if (await rotationRepository(scope).get()) {
            await continueRotation(scope);
            return;
        }
        // The member list is a service response. Each grant must verify against the owner grant of this
        // scope, so the service cannot add a recipient for the next key. One invalid grant stops the plan.
        const members = (await scoped(scope, 'members')).members as SignedGrant[];
        for (const member of members)
            if (member.grant.collection !== scope.collection || !(await verifyGrant(member, scope.owner.grant)))
                throw new Error('Invalid member signature');
        const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? [];
        const revoked = (await db.getMeta<RevokedAccess>(revokedAccessKey(scope))) ?? { members: [], invitations: [] };
        const plan: RotationPlan = {
            v: 1,
            id: newId(),
            revision: 1,
            collection: scope.collection,
            member: scope.member.grant.member,
            target: target.member ? { member: target.member } : { invitation: target.link!.id },
            documents: [],
        };
        const affected = [...sessions.values()].filter(
            (session) =>
                session.record.binding.collection === scope.collection &&
                session.record.binding.member === scope.member.grant.member &&
                (!target.link || session.record.binding.document === target.link.document),
        );
        for (const session of affected) {
            const document = session.record.binding.document,
                epoch = (await scoped(scope, 'epoch', { document })).epoch + 1,
                key = encode(keyBytes());
            const values: RotationDocument['values'] = [],
                updates: RotationDocument['links'] = [];
            // Recipients: members that can read this document, minus the target, earlier revocations,
            // expired grants, and grants that a revoked or targeted invitation issued.
            for (const member of members)
                if (
                    member.grant.member !== target.member &&
                    (!member.grant.document || member.grant.document === document) &&
                    !revoked.members.includes(member.grant.member) &&
                    (!member.issuer || !revoked.invitations.includes(member.issuer.invitation.id)) &&
                    (!member.grant.expires || member.grant.expires > Date.now() / 1000) &&
                    (!target.link || member.issuer?.invitation.id !== target.link.id)
                ) {
                    const wrapped = await wrapKey(
                        { collection: scope.collection, document, epoch, member: member.grant.member },
                        decode(key),
                        scope.identity,
                        member.grant.boxKey,
                    );
                    values.push({ wrapped, signature: await sign(wrapped, scope.identity) });
                }
            // Each remaining link gets a sealed update, so a person who opens the link later reads the current key.
            for (const link of links)
                if (
                    link.document === document &&
                    link.id !== target.link?.id &&
                    !revoked.invitations.includes(link.id) &&
                    link.expires > Date.now() / 1000
                ) {
                    // Unsupported links receive no new document key. Their records stay
                    // available so the owner can explicitly disable them.
                    if (!validLinkWrappingKey(link)) continue;
                    const value = {
                        document,
                        epoch,
                        ...(await sealJson({ epoch, key }, linkWrappingKey(link), { invitation: link.id })),
                    };
                    updates.push({ invitation: link.id, value, signature: await sign(value, scope.identity) });
                }
            plan.documents.push({ document, epoch, key, values, links: updates });
        }
        // Save the exact packets first. The `null` revision makes the commit fail when a plan already exists.
        await rotationRepository(scope).commit(null, plan);
        await continueRotation(scope);
    } catch (error) {
        phase(scope, 'attention');
        viewer.toast(
            `Revocation needs attention: ${errorText(error)}. Publication is paused and local work is retained.`,
        );
    }
}
/**
 * Runs the saved rotation plan of `scope` to completion, or joins the run that is active in this tab.
 *
 * The run holds a Web Lock for the scope where the browser supports locks, so two tabs do not run one plan.
 * It stops the transport, then `resumeRotation` continues from the last saved step: revocation fence, key
 * delivery, epoch activation, and the local key change. The completed plan is removed, and the target
 * enters the local revocation record in the same write. Then the transport restarts and uploads.
 *
 * @returns A promise that never rejects. A failure sets the scope to `attention`, shows a toast, and keeps
 * the plan for a retry.
 */
export async function continueRotation(scope: Scope) {
    if (rotationTasks.has(scopeId(scope))) return rotationTasks.get(scopeId(scope));
    const run = async () => {
        driverFor(scope)?.stop();
        await resumeRotation(
            rotationRepository(scope),
            (action, values) => scoped(scope, action, values),
            // Local step for one document. The session adopts the key first. The key then goes to the
            // scope for the root document, or to the folder manifest for a child.
            async ({ document, epoch, key }) => {
                const session = [...sessions.values()].find(
                    (session) =>
                        session.record.binding.document === document &&
                        session.record.binding.member === scope.member.grant.member,
                );
                if (!session) throw new Error('Restore the local replica before resuming key rotation');
                if (session.record.binding.epoch > epoch)
                    throw new Error('A newer rotation already changed this document');
                await session.rekey(epoch, key, true);
                if (document === scope.root) {
                    scope.key = key;
                    await saveScope(scope);
                }
                if (scope.kind === 'folder' && document !== scope.root) {
                    const root = sessions.get(scope.localRoot)!,
                        old = root.manifest().find((value) => value.id === document);
                    if (old) await root.editManifest([{ ...old, key, epoch }]);
                }
            },
            // Before any request: each saved link packet must open with the wrapping key of its owned link
            // and must hold the planned epoch and key. This also covers a plan from a recovery file.
            async (plan) =>
                validateRotationLinks(plan, (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? []),
        );
        start(scope);
        driverFor(scope)?.retry();
        await driverFor(scope)?.upload();
        viewer.toast('Access revoked and document keys rotated');
    };
    // The lock name is the storage key of the plan, so each tab waits on the same lock.
    const task = ('locks' in navigator ? navigator.locks.request(rotationKey(scope), run) : run())
        .catch((error) => {
            phase(scope, 'attention');
            viewer.toast(`Revocation needs attention: ${errorText(error)}. Use Retry to resume the saved key updates.`);
        })
        .finally(() => rotationTasks.delete(scopeId(scope)));
    rotationTasks.set(scopeId(scope), task);
    return task;
}
