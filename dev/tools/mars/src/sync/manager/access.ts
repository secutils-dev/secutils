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
export async function disconnect(scope: Scope, nodes: TreeNode[]) {
    driverFor(scope)?.stop();
    for (const item of nodes)
        if (!scope.exclusions.includes(item.connection!.document)) scope.exclusions.push(item.connection!.document);
    await saveScope(scope);
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
export async function deletionInfo(ids: string[]) {
    await initialize();
    const selected = app.state.nodes.filter((node) => ids.includes(node.id)),
        now = Date.now() / 1000;
    let liveLinks = 0,
        synced = 0;
    for (const scope of scopes.values()) {
        const nodes = selected.filter((node) => belongs(node, scope) && node.connection!.phase !== 'ended');
        if (!nodes.length) continue;
        if (!scope.live && (scope.personalSync !== false || scope.kind === 'folder'))
            synced += nodes.filter((node) => node.kind === 'doc').length;
        const revoked = await db.getMeta<RevokedAccess>(revokedAccessKey(scope));
        const documents = new Set(nodes.map((node) => node.connection!.document));
        const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? [];
        liveLinks += links.filter(
            (link) => documents.has(link.document) && link.expires > now && !revoked?.invitations.includes(link.id),
        ).length;
    }
    return { liveLinks, synced };
}
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
export async function revoke(scope: Scope, target: { member?: string; link?: OwnedLink }) {
    await viewer.flush();
    driverFor(scope)?.stop();
    try {
        if (await rotationRepository(scope).get()) {
            await continueRotation(scope);
            return;
        }
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
        await rotationRepository(scope).commit(null, plan);
        await continueRotation(scope);
    } catch (error) {
        phase(scope, 'attention');
        viewer.toast(
            `Revocation needs attention: ${errorText(error)}. Publication is paused and local work is retained.`,
        );
    }
}
export async function continueRotation(scope: Scope) {
    if (rotationTasks.has(scopeId(scope))) return rotationTasks.get(scopeId(scope));
    const run = async () => {
        driverFor(scope)?.stop();
        await resumeRotation(
            rotationRepository(scope),
            (action, values) => scoped(scope, action, values),
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
            async (plan) =>
                validateRotationLinks(plan, (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? []),
        );
        start(scope);
        driverFor(scope)?.retry();
        await driverFor(scope)?.upload();
        viewer.toast('Access revoked and document keys rotated');
    };
    const task = ('locks' in navigator ? navigator.locks.request(rotationKey(scope), run) : run())
        .catch((error) => {
            phase(scope, 'attention');
            viewer.toast(`Revocation needs attention: ${errorText(error)}. Use Retry to resume the saved key updates.`);
        })
        .finally(() => rotationTasks.delete(scopeId(scope)));
    rotationTasks.set(scopeId(scope), task);
    return task;
}
