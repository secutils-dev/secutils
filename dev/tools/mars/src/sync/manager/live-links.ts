import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { newId, type TreeNode } from '../../tree/model.ts';
import { esc, modal } from '../../ui/ui.ts';
import { encodeState, decodeState } from '../../share/codec.ts';
import { identity, randomSecret, tokenHash, sign, signGrant, verify, sealJson } from '../crypto.ts';
import {
    encode,
    decode,
    validInvitation,
    type SignedGrant,
    type SignedInvitation,
    type SyncBinding,
} from '../protocol.ts';
import { Session, type SessionRecord } from '../session.ts';
import { BodyReplica } from '../replica.ts';
import { SyncDialog } from '../dialog.ts';
import { icon } from '../../ui/icons.ts';
import {
    isShortLiveLink,
    liveAccess,
    newLiveSecret,
    openLiveBootstrap,
    sealLiveBootstrap,
    linkWrappingKey,
    validLinkWrappingKey,
    type LiveLink,
} from '../live-link.ts';
import {
    rootFor,
    rotationRepository,
    type RevokedAccess,
    revokedAccessKey,
    type OwnedLink,
    scopeId,
    live,
    request,
    errorText,
    phase,
    type Scope,
    belongs,
    scoped,
    saveScope,
    cleanupTasks,
    driverFor,
    scopes,
    sessions,
} from './state.ts';
import { initialize, readLiveKey, createSession, loadPackets, start } from './engine.ts';
import { enrollSelection } from './enrollment.ts';
import { ownerMatchesLink } from './pairing.ts';
import { continueRotation, revoke, stopHere, remove, disconnect } from './access.ts';
import { syncError } from './dialogs.ts';

export async function liveLinks(id: string) {
    await initialize();
    const node = app.node(id),
        scope = node ? rootFor(node) : undefined;
    if (!node || !scope || scope.member.grant.permission !== 'owner') return;
    const ui = new SyncDialog('liveLinksDialog');
    let problem = '';
    try {
        while (ui.open && node.connection) {
            const pending = !!scope.cleanup || !!(await rotationRepository(scope).get());
            const revoked = (await db.getMeta<RevokedAccess>(revokedAccessKey(scope))) ?? {
                members: [],
                invitations: [],
            };
            const links = ((await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? []).filter(
                (link) => link.document === node.connection!.document && !revoked.invitations.includes(link.id),
            );
            const rows = links
                .map(
                    (link, index) =>
                        `<div class="sync-access-item"><span>${esc(link.label ?? `${node.name} - Link ${index + 1}`)}<small>Can ${esc(link.permission ?? 'view')} · ${link.expires > Date.now() / 1000 ? 'Expires' : 'Expired'} ${new Date(link.expires * 1000).toLocaleDateString()}</small></span><button class="icon-btn icon-btn-s" type="button" aria-label="Disable link ${index + 1}" data-tooltip="Disable Live link" data-sync-action="link:${link.id}">${icon('unlink')}</button></div>`,
                )
                .join('');
            const answer = await ui.screen({
                title: 'Live links',
                html: `<p>Links sharing <strong>${esc(node.name)}</strong>.</p><div class="sync-access-list">${rows || '<p>No active Live links.</p>'}</div><p class="sync-fineprint">Disabling a link stops access through that link. Local copies remain. If this document only uses Live links, removing its last link also deletes the remote copy.</p>${pending ? '<p>A sharing change needs to finish. Try again to continue.</p>' : ''}${problem ? `<p role="alert">${esc(problem)}</p>` : ''}`,
                buttons: pending
                    ? [{ label: 'Try again', value: 'retry', primary: true }]
                    : [
                          ...(links.length ? [{ label: 'Stop sharing', value: 'stop', empty: true, left: true }] : []),
                          { label: 'Copy link', value: 'create', primary: true },
                      ],
            });
            problem = '';
            if (answer.value === 'close') break;
            try {
                if (answer.value === 'retry') {
                    ui.busy('Finishing sharing change…');
                    if (scope.cleanup) await continueCleanup(scope, ui.signal);
                    else await continueRotation(scope);
                } else if (answer.value === 'create') await live(id);
                else if (answer.value === 'stop') await stopSharing(scope, node, ui);
                else {
                    const link = links.find((link) => link.id === answer.value.split(':')[1]);
                    if (!link) continue;
                    const confirm = await ui.screen({
                        title: 'Disable this link?',
                        back: true,
                        html: '<p>People using this link will lose access to the Live document. Their local copies remain.</p>',
                        buttons: [{ label: 'Disable link', value: 'disable', danger: true }],
                    });
                    if (confirm.value !== 'disable') continue;
                    ui.busy('Disabling link…');
                    await revoke(scope, { link });
                    if (!(await rotationRepository(scope).get())) await finishSharing(scope, link.document, ui.signal);
                }
            } catch (error) {
                problem = syncError(error);
            }
        }
    } finally {
        ui.close();
    }
}
export async function liveDocument(id: string) {
    await initialize();
    const ui = new SyncDialog('liveDocumentDialog');
    try {
        while (ui.open) {
            const node = app.node(id),
                scope = node ? rootFor(node) : undefined;
            if (!node?.connection || !scope) break;
            const ended = node.connection.phase === 'ended',
                access = { view: 'View only', comment: 'Can comment', edit: 'Can edit', owner: 'Owner' }[
                    scope.member.grant.permission
                ];
            const description =
                scope.live && !validLinkWrappingKey(scope.live)
                    ? 'This link uses an unsupported key format. Ask the owner for a new link. Your local content and pending work stay here.'
                    : ended
                      ? 'The Live link ended. Your retained copy and any unsent changes are still here.'
                      : `Receives changes from the sharing source. Access expires ${new Date(scope.member.grant.expires * 1000).toLocaleDateString()}, unless the owner disables the link first.`;
            const answer = await ui.screen({
                title: 'Live document',
                html: `<p><strong>${esc(node.name)}</strong></p><p><strong>${access}</strong></p><p>${description}</p><p class="sync-fineprint">${ended ? 'Keep as a local copy to make this document independent.' : "Keep as a local copy to stop receiving changes without disabling the owner's link."}</p>`,
                buttons: [
                    { label: 'Keep as local copy', value: 'local', primary: true },
                    ...(db.isTemporaryStorage()
                        ? [{ label: 'Save to library', value: 'save' }]
                        : [{ label: 'Remove from library', value: 'remove', empty: true }]),
                ],
            });
            if (answer.value === 'close') break;
            if (answer.value === 'save') {
                await saveLive(id);
                continue;
            }
            const confirm = await ui.screen({
                title: answer.value === 'local' ? 'Keep as local copy?' : 'Remove from library?',
                back: true,
                html:
                    answer.value === 'local'
                        ? "<p>This document and your current changes stay on this device. This copy will no longer receive Live updates. The owner's link stays active for other people.</p>"
                        : "<p>Remove this copy from your library. This does not disable the owner's Live link.</p>",
                buttons: [
                    {
                        label: answer.value === 'local' ? 'Keep local copy' : 'Remove',
                        value: 'yes',
                        primary: answer.value === 'local',
                        danger: answer.value !== 'local',
                    },
                ],
            });
            if (confirm.value !== 'yes') continue;
            if (answer.value === 'local') await stopHere(id);
            else {
                await remove(id, false);
                await app.remove(id);
            }
        }
    } finally {
        ui.close();
    }
}
function receivedLiveScope(id: string) {
    const node = app.node(id),
        scope = node ? rootFor(node) : undefined;
    return node?.connection?.kind === 'live' &&
        node.connection.phase !== 'ended' &&
        scope?.live &&
        validLinkWrappingKey(scope.live) &&
        scope.member.grant.permission !== 'owner' &&
        scope.member.grant.expires > Date.now() / 1000
        ? scope
        : null;
}
export async function liveLinkInfo(id: string) {
    await initialize();
    const scope = receivedLiveScope(id);
    return scope ? { permission: scope.member.grant.permission, expires: scope.member.grant.expires } : null;
}
export async function forwardLiveLink(id: string) {
    await initialize();
    const scope = receivedLiveScope(id);
    if (!scope) throw new Error('The Live link ended. Choose Snapshot to share your local copy.');
    try {
        await request('invite-read', { invitation: scope.live!.id }, scope.live!.secret);
    } catch (error) {
        if (['invitation-expired', 'connection-ended', 'access-ended'].includes(errorText(error))) {
            phase(scope, 'ended');
            throw new Error('The Live link ended. Choose Snapshot to share your local copy.');
        }
        throw new Error('Could not confirm the Live link. Check your connection and try again.');
    }
    const url =
        scope.liveUrl ?? `${location.origin}${location.pathname}#live=${await encodeState(JSON.stringify(scope.live))}`;
    if (!receivedLiveScope(id)) throw new Error('The Live link ended. Choose Snapshot to share your local copy.');
    return { url, expires: scope.member.grant.expires };
}
async function finishSharing(scope: Scope, document: string, signal?: AbortSignal) {
    const revoked = (await db.getMeta<RevokedAccess>(revokedAccessKey(scope))) ?? { members: [], invitations: [] };
    const links = ((await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? []).filter(
        (link) =>
            link.document === document && link.expires > Date.now() / 1000 && !revoked.invitations.includes(link.id),
    );
    if (links.length) return;
    const nodes = app.state.nodes.filter((node) => belongs(node, scope) && node.connection!.document === document);
    const finish = async () => {
        for (const node of nodes) delete node.connection!.liveShared;
        await db.putNodes(nodes);
        app.notify();
    };
    if (scope.personalSync || scope.kind !== 'doc' || scope.root !== document) {
        await finish();
        return;
    }
    const members = (await scoped(scope, 'members')).members as SignedGrant[];
    if (
        members.some(
            (member) =>
                member.grant.member !== scope.member.grant.member &&
                member.grant.document === null &&
                !revoked.members.includes(member.grant.member),
        )
    ) {
        await finish();
        return;
    }
    // Keep Live links reachable while cleanup is pending, including after close/reload.
    scope.cleanup = { document };
    await saveScope(scope);
    await continueCleanup(scope, signal);
}
export async function continueCleanup(scope: Scope, signal?: AbortSignal) {
    if (cleanupTasks.has(scopeId(scope))) return cleanupTasks.get(scopeId(scope));
    const controller = new AbortController(),
        pause = () => {
            if (document.hidden) controller.abort();
        },
        leave = () => controller.abort();
    document.addEventListener('visibilitychange', pause);
    window.addEventListener('pagehide', leave);
    const task = (async () => {
        driverFor(scope)?.stop();
        while (scope.cleanup) {
            if (document.hidden) throw new DOMException('Cleanup paused', 'AbortError');
            const plan = scope.cleanup;
            let result: { complete: boolean; after?: string };
            try {
                result = await scoped(
                    scope,
                    'purge-live-document',
                    plan,
                    signal ? AbortSignal.any([signal, controller.signal]) : controller.signal,
                );
            } catch (error) {
                if (errorText(error) === 'personal-sync-active') {
                    delete scope.cleanup;
                    scope.personalSync = true;
                    await saveScope(scope);
                    driverFor(scope)?.retry();
                    return;
                }
                if (errorText(error) !== 'connection-expired') throw error;
                result = { complete: true }; // The final cleanup response can be lost after owner metadata is deleted.
            }
            if (result.complete) {
                delete scope.cleanup;
                await disconnect(
                    scope,
                    app.state.nodes.filter((node) => belongs(node, scope)),
                );
                viewer.refreshConnection();
                app.notify();
                viewer.toast('Sharing stopped. The remote copy was deleted. Your local document is retained.');
                return;
            }
            scope.cleanup = { document: plan.document, ...(result.after ? { after: result.after } : {}) };
            await saveScope(scope);
        }
    })().finally(() => {
        document.removeEventListener('visibilitychange', pause);
        window.removeEventListener('pagehide', leave);
        cleanupTasks.delete(scopeId(scope));
    });
    cleanupTasks.set(scopeId(scope), task);
    return task;
}
const resumeCleanup = () => {
    if (!document.hidden)
        for (const scope of scopes.values())
            if (scope.cleanup) void continueCleanup(scope).catch(() => phase(scope, 'attention'));
};
document.addEventListener('visibilitychange', resumeCleanup);
window.addEventListener('pageshow', resumeCleanup);
async function stopSharing(scope: Scope, node: TreeNode, ui: SyncDialog) {
    const answer = await ui.screen({
        title: 'Stop sharing?',
        back: true,
        html: `<p>Disable every Live link for <strong>${esc(node.name)}</strong>. Saved copies stay available. If this document only uses Live links, MARS also deletes its remote copy.</p>`,
        buttons: [{ label: 'Stop sharing', value: 'stop', danger: true }],
    });
    if (answer.value !== 'stop') return;
    ui.busy('Stopping sharing…');
    const revoked = (await db.getMeta<RevokedAccess>(revokedAccessKey(scope))) ?? { members: [], invitations: [] };
    const links = ((await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? []).filter(
        (link) => link.document === node.connection!.document && !revoked.invitations.includes(link.id),
    );
    for (const link of links) {
        await revoke(scope, { link });
        if (await rotationRepository(scope).get()) throw new Error('revocation-pending');
    }
    await finishSharing(scope, node.connection!.document, ui.signal);
}
export async function createLiveLink(
    id: string,
    permission: 'view' | 'comment' | 'edit',
    ttl: number,
    format: 'short' | 'full' = 'short',
) {
    await initialize();
    const node = app.node(id);
    if (!node || node.kind !== 'doc') throw new Error('Choose a document to share live.');
    if (node.connection && rootFor(node)?.member.grant.permission !== 'owner')
        throw new Error('Create Live links from the original sharing device.');
    if (!node.connection) await enrollSelection(node.id, false);
    const scope = rootFor(node),
        session = sessions.get(node.id);
    if (!scope || !session) throw new Error('sync-not-ready');
    await viewer.flush();
    await driverFor(scope)?.upload();
    if (await driverFor(scope)?.options.replica.pending()) throw new Error('sync-pending');
    const delegate = await identity(),
        fragment = newLiveSecret(),
        access = await liveAccess(fragment),
        secret = access.token,
        policy: SignedInvitation = {
            invitation: {
                v: 1,
                id: access.id,
                collection: scope.collection,
                document: node.connection!.document,
                mode: 'live',
                permission,
                publicKey: delegate.publicKey,
                participant: null,
                expires: Math.floor(Date.now() / 1000) + ttl,
            },
            signature: '',
        };
    policy.signature = await sign(policy.invitation, scope.identity);
    const link: LiveLink = {
        v: 1,
        id: policy.invitation.id,
        secret,
        ownerKey: scope.owner.grant.publicKey,
        signing: delegate.signing,
        key: session.record.key,
        epoch: session.record.binding.epoch,
        wrappingKey: encode(access.key),
    };
    await scoped(scope, 'invite', {
        policy,
        tokenHash: await tokenHash(secret),
        bootstrap: await sealLiveBootstrap(link, fragment),
    });
    const url = `${location.origin}${location.pathname}#live=${format === 'full' ? await encodeState(JSON.stringify(link)) : fragment}`;
    node.connection!.liveShared = true;
    await db.putNodes([node]);
    const links = (await db.getMeta<OwnedLink[]>(`sync:links:${scopeId(scope)}`)) ?? [];
    links.push({
        id: policy.invitation.id,
        document: node.connection!.document,
        expires: policy.invitation.expires,
        secret,
        wrappingKey: link.wrappingKey,
        permission,
    });
    await db.setMeta(`sync:links:${scopeId(scope)}`, links);
    await writeLiveKey(
        scope,
        policy.invitation.id,
        link.wrappingKey,
        node.connection!.document,
        session.record.binding.epoch,
        session.record.key,
    );
    return { url, expires: policy.invitation.expires };
}
export async function receiveLive(address: string, encoded: string) {
    if (!db.isTemporaryStorage()) {
        const url = new URL(address);
        url.searchParams.set('temporary', '1');
        location.assign(url);
        return true;
    }
    await initialize();
    try {
        let link: LiveLink, incoming;
        if (isShortLiveLink(encoded)) {
            const access = await liveAccess(encoded);
            incoming = await request('invite-read', { invitation: access.id }, access.token);
            link = await openLiveBootstrap(incoming.bootstrap, encoded);
        } else {
            const raw = await decodeState(encoded);
            if (!raw) throw new Error('Invalid live link');
            link = JSON.parse(raw);
            linkWrappingKey(link);
            incoming = await request('invite-read', { invitation: link.id }, link.secret);
        }
        if (
            !validInvitation(incoming.policy) ||
            !(await ownerMatchesLink(incoming.owner, link.ownerKey)) ||
            incoming.policy.invitation.mode !== 'live' ||
            !(await verify(incoming.policy.invitation, incoming.policy.signature, link.ownerKey))
        )
            throw new Error('Invalid live-link signature');
        const policy: SignedInvitation = incoming.policy,
            who = await identity(),
            token = randomSecret(),
            memberId = newId();
        const updated = incoming.keyUpdate
            ? await readLiveKey(incoming.keyUpdate, link, incoming.owner)
            : { epoch: link.epoch ?? 1, key: link.key };
        const grant = {
            v: 1 as const,
            collection: policy.invitation.collection,
            member: memberId,
            participant: memberId,
            permission: policy.invitation.permission,
            document: policy.invitation.document,
            publicKey: who.publicKey,
            boxKey: who.boxKey,
            tokenHash: await tokenHash(token),
            expires: policy.invitation.expires,
        };
        const signed = await signGrant(grant, {
            signing: link.signing,
            box: {},
            publicKey: policy.invitation.publicKey,
            boxKey: '',
        });
        signed.issuer = policy;
        await request('live-join', { invitation: link.id, signed }, link.secret);
        const id = newId(),
            node: TreeNode = {
                id,
                parentId: null,
                kind: 'doc',
                name: 'Live document',
                order: 0,
                createdAt: Date.now(),
                updatedAt: Date.now(),
            };
        app.state.nodes.push(node);
        const scope: Scope = {
            collection: grant.collection,
            root: grant.document!,
            localRoot: id,
            kind: 'doc',
            key: updated.key,
            token,
            owner: incoming.owner,
            member: signed,
            identity: who,
            exclusions: [],
            live: link,
            liveUrl: address,
        };
        await saveScope(scope);
        const binding: SyncBinding = {
            collection: grant.collection,
            document: grant.document!,
            root: grant.document!,
            member: memberId,
            permission: grant.permission,
            kind: 'live',
            epoch: updated.epoch,
        };
        const record = Session.seed(binding, '', who, token, updated.key, incoming.owner, node.name, signed),
            empty = new BodyReplica();
        record.body = encode(empty.checkpoint());
        empty.destroy();
        record.keys = { [String(link.epoch ?? 1)]: link.key, [String(updated.epoch)]: updated.key };
        const session = await createSession(node, record);
        await loadPackets(scope, session);
        node.name = session.metadata().name ?? node.name;
        start(scope);
        await app.openDoc(id);
        viewer.endLoading();
        app.notify();
    } catch (error) {
        viewer.endLoading();
        viewer.toast(
            `Could not open the live link: ${errorText(error) === 'unsupported-link-key-format' ? syncError(error) : errorText(error)}`,
        );
    }
    return true;
}
export async function saveLive(id: string) {
    await viewer.flush();
    const node = app.node(id),
        session = sessions.get(id),
        scope = node ? rootFor(node) : undefined;
    if (!node || !session || !scope || node.connection?.kind !== 'live') return;
    const ended = node.connection.phase === 'ended';
    const answer = await modal({
        title: ended ? 'Save local copy to library' : 'Save live document to library',
        body: ended
            ? 'Keep this copy and any unsent changes on this device so you can reopen it later. The old Live link remains disabled. Other temporary documents are not saved.'
            : 'Keep this document and any unsent changes on this device so you can reopen it later. It stays connected while the Live link is active. Other temporary documents are not saved.',
        buttons: [
            { label: 'Cancel', value: '' },
            { label: 'Save to library', value: 'yes', primary: true },
        ],
    });
    if (answer.value !== 'yes') return;
    const record = await db.getSide<SessionRecord>('sync', id);
    if (!record) return;
    try {
        await db.promoteConnected(node, record.projection, record, { [scopeId(scope)]: scope, lastDoc: id });
        await app.load();
        await initialize(true);
        history.replaceState(history.state, '', `${location.pathname}#doc=${id}`);
        app.notify();
        viewer.toast('Live document saved to library');
    } catch (error) {
        viewer.toast(`The live session is still temporary: ${errorText(error)}. Download pending work before closing.`);
    }
}
async function writeLiveKey(
    scope: Scope,
    invitation: string,
    wrappingKey: string,
    document: string,
    epoch: number,
    key: string,
) {
    const encrypted = await sealJson({ epoch, key }, decode(wrappingKey), { invitation });
    const value = { document, epoch, ...encrypted };
    await scoped(scope, 'live-key-write', { invitation, value, signature: await sign(value, scope.identity) });
}
