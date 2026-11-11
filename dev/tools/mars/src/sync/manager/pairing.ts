import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { newId, nextOrder, textBytes, type TreeNode } from '../../tree/model.ts';
import { esc, modal, copyRich } from '../../ui/ui.ts';
import { mergeDocuments, openThreads } from '../../share/merge.ts';
import { matchScore } from '../../share/match.ts';
import { decodeState } from '../../share/codec.ts';
import {
    identity,
    randomSecret,
    tokenHash,
    sign,
    signGrant,
    wrapKey,
    unwrapKey,
    verify,
    verifyGrant,
    pairingPhrase,
    sealJson,
    openJson,
    hash,
} from '../crypto.ts';
import {
    encode,
    decode,
    canonical,
    jsonBytes,
    LIMITS,
    validId,
    validGrant,
    validInvitation,
    validWrappedKey,
    type SignedGrant,
    type SignedInvitation,
    type WrappedKey,
    type SyncBinding,
} from '../protocol.ts';
import { Session, stagedSession } from '../session.ts';
import { BodyReplica } from '../replica.ts';
import { SyncDialog } from '../dialog.ts';
import { copyFeedback } from '../../ui/feedback.ts';
import { linkWrappingKey } from '../live-link.ts';
import {
    isShortSyncLink,
    newSyncSecret,
    openSyncBootstrap,
    sealSyncBootstrap,
    syncAccess,
    type PairLink,
} from '../sync-link.ts';
import {
    request,
    type Scope,
    saveScope,
    driverFor,
    scoped,
    sessions,
    scopeId,
    scopes,
    errorText,
    belongs,
    repository,
} from './state.ts';
import { initialize, loadPackets, materialize, start, validateSnapshot } from './engine.ts';
import { receiveLive } from './live-links.ts';
import { syncError, confirmationCode } from './dialogs.ts';

async function waitForPair(
    action: string,
    values: Record<string, unknown>,
    token: string,
    controller: AbortController,
    ready: (value: Record<string, unknown>) => boolean,
) {
    let read: AbortController | null = null;
    const pause = () => {
        if (document.hidden || !navigator.onLine || controller.signal.aborted) read?.abort();
    };
    const leave = () => controller.abort();
    document.addEventListener('visibilitychange', pause);
    window.addEventListener('offline', pause);
    window.addEventListener('pagehide', leave);
    controller.signal.addEventListener('abort', pause);
    try {
        while (!controller.signal.aborted) {
            if (!document.hidden && navigator.onLine) {
                read = new AbortController();
                try {
                    const result = await request(action, values, token, read.signal);
                    if (!controller.signal.aborted && !read.signal.aborted && ready(result)) return result;
                } catch (error) {
                    if ((error as Error)?.name !== 'AbortError') throw error;
                } finally {
                    read = null;
                }
            }
            await new Promise((resolve) => setTimeout(resolve, 1000));
        }
    } finally {
        read?.abort();
        document.removeEventListener('visibilitychange', pause);
        window.removeEventListener('offline', pause);
        window.removeEventListener('pagehide', leave);
        controller.signal.removeEventListener('abort', pause);
    }
}
/** The longest device name that a pairing request carries. */
const MAX_DEVICE_NAME = 64;
/**
 * Authenticated data for the encrypted device name of a pairing request. It binds the requested
 * grant to the link key, which the service never receives.
 */
const pairLabelContext = async (invitation: string, grant: SignedGrant['grant']) => ({
    invitation,
    grant: await hash(jsonBytes(grant)),
});
/** True when `owner` is a well-formed, self-signed owner grant for the owner key of a link. */
export async function ownerMatchesLink(owner: unknown, ownerKey: string): Promise<boolean> {
    return (
        validGrant(owner) &&
        !owner.issuer &&
        owner.grant.permission === 'owner' &&
        owner.grant.publicKey === ownerKey &&
        verify(owner.grant, owner.signature, ownerKey)
    );
}
export async function syncLinkPanel(scope: Scope, ui: SyncDialog) {
    if (!ui.open) return;
    if (scope.selection) {
        await ui.screen({
            title: 'Connect a device',
            back: true,
            html: '<p>Open the original synced folder to connect another device. This recovery contains selected documents only.</p>',
        });
        return;
    }
    scope.personalSync = true;
    await saveScope(scope);
    ui.busy('Creating Sync link…');
    await viewer.flush();
    await driverFor(scope)?.upload();
    if (await driverFor(scope)?.options.replica.pending()) {
        await ui.screen({
            title: 'Sync link',
            back: true,
            html: '<p>Your latest changes have not finished syncing. Go back and try again before connecting another device.</p>',
        });
        return;
    }
    const fragment = newSyncSecret(),
        access = await syncAccess(fragment),
        secret = access.token,
        invitation = {
            v: 1 as const,
            id: access.id,
            collection: scope.collection,
            document: null,
            mode: 'pair' as const,
            permission: 'edit' as const,
            publicKey: scope.identity.publicKey,
            participant: scope.member.grant.participant,
            expires: Math.floor(Date.now() / 1000) + LIMITS.invitationSeconds,
        };
    const policy: SignedInvitation = { invitation, signature: await sign(invitation, scope.identity) };
    const link: PairLink = {
        v: 1,
        id: invitation.id,
        secret,
        ownerKey: scope.owner.grant.publicKey,
        wrappingKey: encode(access.key),
    };
    await scoped(scope, 'invite', {
        policy,
        tokenHash: await tokenHash(secret),
        bootstrap: await sealSyncBootstrap(link, fragment),
    });
    const url = `${location.origin}${location.pathname}#sync=${fragment}`;
    const controller = new AbortController(),
        close = () => controller.abort();
    ui.signal.addEventListener('abort', close, { once: true });
    let pending: Record<string, unknown> | undefined,
        problem = '';
    const name = app.node(scope.localRoot)?.name ?? 'this document';
    const screen = ui.screen({
        title: 'Sync link',
        back: true,
        html: `<p>Open this link on your other device to sync <strong>${esc(name)}</strong>. Confirm the matching code here to approve the connection.</p><p class="sync-fineprint">The link expires in 10 minutes. Keep this window open until your other device connects.</p><p class="sync-fineprint">The other device connects as you and can edit this ${scope.kind === 'folder' ? 'folder' : 'document'}. For other people, use <strong>Share → Copy link</strong> and choose Live to set their access.</p>`,
        field: { label: 'Sync link', value: url, readonly: true },
        buttons: [
            {
                label: 'Copy link',
                value: 'copy',
                primary: true,
                run: (button) =>
                    copyRich(url)
                        .then(() => copyFeedback(button))
                        .catch(() => copyFeedback(button, 'Could not copy. Select the link and copy it manually.')),
            },
        ],
    });
    const waiting = waitForPair(
        'pair-pending',
        { collection: scope.collection, member: scope.member.grant.member, invitation: invitation.id },
        scope.token,
        controller,
        (value) => !!value.candidate,
    )
        .then((value) => {
            pending = value;
            if (!controller.signal.aborted) ui.resolve(value?.candidate ? 'candidate' : 'back');
        })
        .catch((error) => {
            if (!controller.signal.aborted) {
                problem = syncError(error);
                ui.resolve('error');
            }
        });
    try {
        const result = await screen;
        controller.abort();
        await waiting;
        if (result.value === 'error' && ui.open) {
            await ui.screen({ title: 'Sync link', back: true, html: `<p role="alert">${esc(problem)}</p>` });
            return;
        }
        if (result.value !== 'candidate' || !pending?.candidate || !ui.open) return;
        // The request arrives through the service, so it is untrusted until two checks pass. The grant
        // must be exactly what a Sync link permits. The device name must decrypt under the link key
        // with this grant as authenticated data: the service never sees that key, so it cannot swap
        // in a grant of its own. The code that both screens show is the final check by the user.
        const candidate = pending.candidate as SignedGrant;
        let label: { name: string };
        try {
            if (
                !validGrant(candidate) ||
                candidate.issuer ||
                candidate.grant.collection !== scope.collection ||
                candidate.grant.permission !== 'edit' ||
                candidate.grant.document !== null ||
                candidate.grant.participant !== invitation.participant ||
                candidate.grant.expires !== 0 ||
                !(await verify(candidate.grant, candidate.signature, candidate.grant.publicKey))
            )
                throw new Error('invalid-pair-request');
            label = await openJson<{ name: string }>(
                pending.label as { nonce: string; ciphertext: string },
                access.key,
                await pairLabelContext(invitation.id, candidate.grant),
            );
            if (typeof label?.name !== 'string') throw new Error('invalid-pair-request');
        } catch {
            await ui.screen({
                title: 'Sync link',
                back: true,
                html: '<p role="alert">The request from the other device is not valid. Reload MARS on both devices, then create a new Sync link.</p>',
            });
            return;
        }
        const phrase = await pairingPhrase(policy, candidate.grant);
        const answer = await ui.screen({
            title: 'Check the code',
            back: true,
            html: `<p><strong>${esc(label.name)}</strong> is ready to connect.</p><p>Does your other device show this same code?</p>${confirmationCode(phrase)}`,
            buttons: [{ label: 'Codes match', value: 'yes', primary: true }],
        });
        if (answer.value !== 'yes') return;
        const rootEpoch = sessions.get(scope.localRoot)?.record.binding.epoch ?? 1;
        const signed = await signGrant(candidate.grant, scope.identity),
            keys = [
                await wrapKey(
                    {
                        collection: scope.collection,
                        document: scope.root,
                        epoch: rootEpoch,
                        member: signed.grant.member,
                    },
                    decode(scope.key),
                    scope.identity,
                    candidate.grant.boxKey,
                ),
            ];
        ui.busy('Connecting your device…');
        await scoped(scope, 'pair-approve', { invitation: invitation.id, signed, keys });
        scope.clientNames = { ...scope.clientNames, [signed.grant.member]: label.name };
        await saveScope(scope);
    } finally {
        controller.abort();
        ui.signal.removeEventListener('abort', close);
    }
}
export async function receiveLocation(address: string) {
    const params = new URLSearchParams(new URL(address).hash.slice(1));
    if (params.has('live')) return receiveLive(address, params.get('live')!);
    const encoded = params.get('sync') ?? params.get('invite'); // Accept links issued before #sync=.
    if (encoded === null) return false;
    await initialize();
    history.replaceState(history.state, '', location.pathname);
    let receiving: SyncDialog | undefined;
    const leave = () => receiving?.close();
    try {
        let link: PairLink, incoming;
        if (isShortSyncLink(encoded)) {
            const access = await syncAccess(encoded);
            incoming = await request('invite-read', { invitation: access.id }, access.token);
            link = await openSyncBootstrap(incoming.bootstrap, encoded);
        } else {
            const decoded = await decodeState(encoded);
            if (!decoded) throw new Error('Invalid Sync link');
            link = JSON.parse(decoded);
            linkWrappingKey(link);
            incoming = await request('invite-read', { invitation: link.id }, link.secret);
        }
        if (
            !validInvitation(incoming.policy) ||
            !(await ownerMatchesLink(incoming.owner, link.ownerKey)) ||
            !(await verify(incoming.policy.invitation, incoming.policy.signature, link.ownerKey))
        )
            throw new Error('Invalid Sync link');
        const answer = await modal({
            title: 'Sync',
            html: '<p>Connect this device to your synced document or folder. Your other documents stay separate.</p><p>Give this device a name so you can recognize it later.</p>',
            field: { label: 'Name', value: 'My device' },
            buttons: [
                { label: 'Cancel', value: '' },
                { label: 'Connect', value: 'yes', primary: true },
            ],
        });
        if (answer.value !== 'yes') return true;
        const who = await identity(),
            token = randomSecret(),
            g = {
                v: 1 as const,
                collection: incoming.policy.invitation.collection,
                member: newId(),
                participant: incoming.policy.invitation.participant,
                permission: 'edit' as const,
                document: null,
                publicKey: who.publicKey,
                boxKey: who.boxKey,
                tokenHash: await tokenHash(token),
                expires: 0,
            };
        const candidate = await signGrant(g, who);
        const encryptedName = await sealJson(
            { name: answer.text.trim().slice(0, MAX_DEVICE_NAME) || 'My device' },
            linkWrappingKey(link),
            await pairLabelContext(link.id, g),
        );
        await request('pair-request', { invitation: link.id, candidate, label: encryptedName }, link.secret);
        const phrase = await pairingPhrase(incoming.policy, candidate.grant),
            controller = new AbortController();
        const waiting = modal({
            title: 'Sync',
            html: `<p>Check that your other device shows this same code:</p>${confirmationCode(phrase)}<p role="status">Confirm the code there to finish connecting.</p>`,
            buttons: [{ label: 'Cancel', value: '' }],
        });
        waiting.then(() => controller.abort());
        const result = (await waitForPair(
            'pair-result',
            { invitation: link.id },
            link.secret,
            controller,
            (value) => !!value.approved,
        )) as
            | {
                  approved: boolean;
                  signed: SignedGrant;
                  owner: SignedGrant;
                  keys: WrappedKey[];
                  root: { document: string; kind: 'doc' | 'folder' };
              }
            | undefined;
        if (!result?.approved || controller.signal.aborted) return true;
        document.querySelector<HTMLDialogElement>('#marsModal')?.close();
        await waiting;
        const ui = (receiving = new SyncDialog('syncDownloadDialog')),
            folder = result.root.kind === 'folder';
        void ui
            .screen({
                title: folder ? 'Downloading synced folder' : 'Downloading synced document',
                html: `<p id="syncDownloadName">${folder ? 'Preparing your synced folder.' : 'Preparing your synced document.'}</p><progress class="sync-download-progress" aria-label="Sync download" aria-describedby="syncDownloadStatus"></progress><p id="syncDownloadStatus" role="status" aria-atomic="true">${folder ? 'Reading folder details…' : 'Downloading document…'}</p><p class="sync-fineprint">Keep this tab open. ${folder ? 'The folder will appear with its complete hierarchy when the download finishes.' : 'The document will appear when the download finishes.'}</p>`,
                buttons: [{ label: 'Cancel', value: 'cancel' }],
            })
            .then(() => ui.close());
        window.addEventListener('pagehide', leave, { once: true });
        const meter = ui.element.querySelector('progress')!,
            status = ui.element.querySelector('#syncDownloadStatus')!;
        const progress = (completed: number, total: number) => {
            if (!ui.open) return;
            if (total) {
                meter.max = total;
                meter.value = completed;
            }
            status.textContent = total
                ? `${completed} of ${total} ${total === 1 ? 'document' : 'documents'} downloaded${completed === total ? '. Saving folder…' : '…'}`
                : 'Saving folder hierarchy…';
        };
        // The approval also arrives through the service. Accept it only when the owner key from the
        // link signed exactly the grant that this device requested, and wrapped the root key itself.
        const wrapped = Array.isArray(result.keys) ? result.keys[0] : undefined;
        if (
            !(await ownerMatchesLink(result.owner, link.ownerKey)) ||
            !validGrant(result.signed) ||
            result.signed.issuer ||
            canonical(result.signed.grant) !== canonical(g) ||
            !(await verifyGrant(result.signed, result.owner.grant)) ||
            !validWrappedKey(wrapped) ||
            wrapped.sender !== result.owner.grant.boxKey ||
            wrapped.collection !== g.collection ||
            wrapped.member !== g.member ||
            !validId(result.root?.document) ||
            wrapped.document !== result.root.document ||
            !['doc', 'folder'].includes(result.root.kind)
        )
            throw new Error('Invalid approval');
        const rootKey = await unwrapKey(result.keys[0], who),
            root = result.root;
        const local = newId(),
            node: TreeNode = {
                id: local,
                parentId: null,
                kind: root.kind,
                name: 'Sync',
                order: nextOrder(app.state.nodes, null),
                createdAt: Date.now(),
                updatedAt: Date.now(),
            };
        const scope: Scope = {
            collection: g.collection,
            root: root.document,
            localRoot: local,
            kind: root.kind,
            token,
            key: encode(rootKey),
            owner: result.owner,
            member: result.signed,
            identity: who,
            exclusions: [],
        };
        const binding: SyncBinding = {
            collection: g.collection,
            document: root.document,
            member: g.member,
            permission: 'edit',
            kind: 'sync',
            epoch: result.keys[0].epoch,
            root: root.document,
            participant: scope.member.grant.participant,
            ownerParticipant: scope.owner.grant.participant,
        };
        const record = Session.seed(binding, '', who, token, scope.key, result.owner, 'Sync');
        record.member = result.signed;
        const empty = new BodyReplica();
        record.body = encode(empty.checkpoint());
        empty.destroy();
        const session = stagedSession(record);
        try {
            await loadPackets(scope, session, ui.signal, true);
            node.name = session.metadata().name ?? 'Sync';
            ui.element.querySelector('#syncDownloadName')!.textContent = node.name;
            node.connection = { ...session.record.binding, phase: 'working' };
            if (root.kind === 'folder') await materialize(scope, session, ui.signal, { bootstrap: node, progress });
            else {
                meter.max = meter.value = 1;
                status.textContent = 'Saving document…';
                const meta = session.metadata();
                Object.assign(node, {
                    threads: openThreads(session.record.projection),
                    size: textBytes(session.record.projection),
                    ...(validId(meta.origin) ? { originId: meta.origin } : {}),
                    ...(meta.readingWidth === 'full' || typeof meta.readingWidth === 'number'
                        ? { readingWidth: meta.readingWidth }
                        : {}),
                });
                await app.serializePersistence(async () => {
                    ui.signal.throwIfAborted();
                    await db.materializeConnected(node, session.record.body, [], {
                        record: session.record,
                        metadata: { [scopeId(scope)]: scope },
                    });
                    await app.load();
                });
                scopes.set(scopeId(scope), scope);
                await initialize(true);
            }
        } finally {
            session.destroy();
        }
        ui.close();
        await offerLocalMatches(scope);
        start(scope);
        if (root.kind === 'doc') await app.openDoc(scope.localRoot);
        else {
            app.state.folderId = scope.localRoot;
            await app.setOpen(scope.localRoot, true);
        }
        app.notify();
        viewer.toast('Sync connected');
    } catch (error) {
        viewer.toast(
            (error as Error)?.name === 'AbortError'
                ? 'Sync setup canceled.'
                : errorText(error) === 'Invalid Sync link'
                  ? 'This Sync link is invalid. Get a new one from your other device.'
                  : syncError(error),
        );
    } finally {
        window.removeEventListener('pagehide', leave);
        receiving?.close();
    }
    return true;
}
async function offerLocalMatches(scope: Scope) {
    for (const incoming of app.state.nodes.filter((node) => node.kind === 'doc' && belongs(node, scope))) {
        const session = sessions.get(incoming.id);
        if (!session) continue;
        const remote = session.record.projection;
        let best: TreeNode | undefined,
            localText = '',
            score = 0;
        for (const local of app.state.nodes.filter((node) => node.kind === 'doc' && !node.connection)) {
            const text = (await db.getText(local.id)) ?? '';
            const match =
                local.originId && local.originId === incoming.originId
                    ? 2
                    : matchScore({ name: incoming.name, text: remote }, { name: local.name, text });
            if (match > score) {
                best = local;
                localText = text;
                score = match;
            }
        }
        if (!best) continue;
        const merged = mergeDocuments(localText, remote);
        const answer = await modal({
            title: 'Connect the existing copy?',
            html: `<p><strong>${esc(incoming.name)}</strong> matches <strong>${esc(best.name)}</strong> in this library.</p>
            <p>Names and content are matching hints, not Sync identities. Keep the copies separate, or connect the existing document.</p>
            <p>Connecting retains its local ID and moves it into the selected collection. Different local content stays in an independent backup.</p>`,
            buttons: [
                { label: 'Keep copies separate', value: '', primary: true },
                { label: 'Connect with my text', value: 'mine' },
                ...(!merged.sameBody ? [{ label: 'Connect with Sync text', value: 'theirs' }] : []),
            ],
        });
        if (!answer.value) continue;
        const next = answer.value === 'mine' ? merged.mine : merged.theirs;
        try {
            await validateSnapshot(session, next);
            await session.change(session.record.projection, next);
            const record = await session.repository.get();
            if (!record) throw new Error('The incoming replica is unavailable');
            const connected = {
                ...best,
                parentId: incoming.parentId,
                name: incoming.name,
                connection: incoming.connection,
                originId: incoming.originId,
                localReadingWidth: best.localReadingWidth ?? best.readingWidth,
                manualFileSave: true,
                ...(app.bodyOf(localText) !== app.bodyOf(next) ? { pending: true as const } : {}),
                ...(best.handle && localText !== next ? { dirty: true } : {}),
            };
            const backup: TreeNode | undefined =
                localText !== next
                    ? {
                          id: newId(),
                          originId: newId(),
                          name: app.uniqueName(`${best.name} (before Sync)`, best.parentId),
                          kind: 'doc',
                          parentId: best.parentId,
                          order: nextOrder(app.state.nodes, best.parentId),
                          createdAt: Date.now(),
                          updatedAt: Date.now(),
                          readingWidth: best.readingWidth,
                      }
                    : undefined;
            const updated = { ...scope, localRoot: scope.localRoot === incoming.id ? best.id : scope.localRoot };
            await db.adoptConnected(
                incoming.id,
                best,
                connected,
                localText,
                record,
                { [scopeId(scope)]: updated },
                backup,
            );
            session.destroy();
            sessions.delete(incoming.id);
            Object.assign(scope, updated);
            await app.load();
            sessions.set(best.id, new Session(record, repository(best.id)));
        } catch (error) {
            viewer.toast(`The copies stay separate: ${errorText(error)}. No existing local document was overwritten.`);
        }
    }
}
