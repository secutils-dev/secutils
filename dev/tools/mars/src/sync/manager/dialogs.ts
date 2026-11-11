import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { descendants } from '../../tree/model.ts';
import { esc } from '../../ui/ui.ts';
import { LIMITS } from '../protocol.ts';
import type { SessionRecord } from '../session.ts';
import { SyncDialog } from '../dialog.ts';
import { icon, type IconName } from '../../ui/icons.ts';
import { syncDebug, safeError } from '../debug.ts';
import { errorText, rootFor, sessions, belongs, saveScope, rotationRepository, driverFor } from './state.ts';
import { initialize, start } from './engine.ts';
import { SyncSetupError, enrollSelection, canRestartUnfinished } from './enrollment.ts';
import { syncLinkPanel } from './pairing.ts';
import { liveDocument, liveLinks, continueCleanup } from './live-links.ts';
import { disconnect, accessPanel, continueRotation, stopHere } from './access.ts';

export const enroll = (id: string) => syncDialog(id);
export async function details(id: string) {
    await initialize();
    const node = app.node(id);
    if (node?.connection?.kind === 'live') return liveDocument(id);
    return node?.connection?.liveShared ? liveLinks(id) : syncDialog(id);
}
export const syncError = (error: unknown) =>
    ({
        'service-unavailable':
            'The Sync service is unavailable right now. Your documents are still here. Try again later.',
        'binding-missing':
            'This tab lost the saved connection during setup. MARS has not reset your library. Try again.',
        'orphan-replica':
            'An earlier setup left saved connection data without a connected item. Your saved documents stay unchanged.',
        'already-connected':
            'Some selected items are already connected. Open Sync for those items before connecting the whole folder.',
        'selection-too-large': `This selection has more than ${LIMITS.documents} items. Choose a smaller folder.`,
        'temporary-storage': 'Save this document to your library before turning on Sync.',
        'sync-pending':
            'Your connection is saved, but its first upload has not finished. Keep this tab visible and check your connection, then try again.',
        'selection-grown':
            'The folder contents changed during setup. Your documents are still here. Try again with the current folder.',
        'missing-node': 'The selected item was removed during setup. Your other documents are still here.',
        'text-changed': 'A document changed during setup. Your latest text is still saved here. Try again.',
        'metadata-changed':
            'A name, location, or document setting changed during setup. Your documents are still here. Try again.',
        revision:
            'Another tab changed the saved connection during setup. Your local documents are still here. Try again.',
        'collection-creation-limit':
            'Too many new sync connections from this network. Try again later. Your documents stay here.',
        'sync-storage-full': 'Sync storage is full. Your changes are saved here. Try again later.',
        'update-too-large':
            'This document is too large to sync. Your changes are saved here. Download a copy to keep them.',
        'invitation-expired': 'This Sync link expired. Go back and create a new one.',
        'unsupported-link-key-format': 'Unsupported link key format. Create a new link from the sharing device.',
    })[error instanceof SyncSetupError ? error.code : errorText(error)] ??
    'Could not finish. Your document is still here. Try again.';
const syncTool = (label: string, action: string, glyph: IconName) =>
    `<button type="button" class="btn" data-sync-action="${action}">${icon(glyph)}<span>${label}</span></button>`;
export const confirmationCode = (phrase: string) =>
    `<div class="sync-code" role="group" aria-label="Confirmation code">${phrase
        .split(' ')
        .map((part) => `<span>${esc(part)}</span>`)
        .join(' ')}</div>`;
async function syncDialog(id: string) {
    await initialize();
    const ui = new SyncDialog();
    let openSyncLink = false,
        problem = '',
        stopped = false;
    let restart: db.UnfinishedSetupSnapshot | undefined;
    try {
        while (ui.open) {
            try {
                if (restart) {
                    const previous = restart;
                    restart = undefined;
                    ui.busy('Restarting setup…');
                    await enrollSelection(id, true, previous);
                    problem = '';
                    openSyncLink = true;
                    continue;
                }
                const node = app.node(id);
                if (!node) break;
                const scope = rootFor(node);
                if (node.connection?.kind === 'live') {
                    ui.close();
                    return liveDocument(id);
                }
                // Older releases exposed the root before finishing its folder manifest.
                // Restart only after an explicit choice, keeping all saved content local.
                if (
                    scope?.kind === 'folder' &&
                    scope.member.grant.permission === 'owner' &&
                    id === scope.localRoot &&
                    !scope.selection &&
                    sessions.has(id) &&
                    !sessions.get(id)!.manifest().length &&
                    !app.state.nodes.some((item) => belongs(item, scope) && item.connection!.liveShared)
                ) {
                    const answer = await ui.screen({
                        title: 'Finish folder setup',
                        html: '<p>The previous Sync setup did not finish. Restart setup to include this folder and its documents. Your local documents and comments stay here.</p><p>Devices connected through the incomplete setup will need a new Sync link.</p>',
                        buttons: [
                            { label: 'Cancel', value: 'close' },
                            { label: 'Restart setup', value: 'restart', primary: true },
                        ],
                    });
                    if (answer.value !== 'restart') break;
                    ui.busy('Restarting setup…');
                    await viewer.flush();
                    const selected = [node, ...descendants(app.state.nodes, id)];
                    for (const item of selected)
                        if (!item.connection) {
                            // A stale tab in the old release could save an unconnected node
                            // over its binding while leaving the atomic projection/outbox.
                            const record = await db.getSide<SessionRecord>('sync', item.id);
                            if (
                                record?.binding.collection === scope.collection &&
                                record.binding.member === scope.member.grant.member
                            )
                                item.connection = { ...record.binding, phase: 'attention' };
                        }
                    await disconnect(
                        scope,
                        selected.filter((item) => belongs(item, scope)),
                    );
                    await enrollSelection(id);
                    problem = '';
                    openSyncLink = true;
                    continue;
                }
                const personal =
                    scope &&
                    (scope.member.grant.permission !== 'owner' ||
                        scope.kind === 'folder' ||
                        scope.personalSync === true ||
                        (scope.personalSync === undefined &&
                            (!node.connection?.liveShared || Object.keys(scope.clientNames ?? {}).length > 0)));
                if (!scope || !personal) {
                    if (db.isTemporaryStorage()) {
                        await ui.screen({
                            title: 'Sync',
                            html: '<p>Save this document to your library before turning on sync.</p>',
                            buttons: [{ label: 'Close', value: 'close' }],
                        });
                        break;
                    }
                    const selection =
                        node.kind === 'folder' ? `this folder and the documents inside it` : 'this document';
                    const answer = await ui.screen({
                        title: 'Sync',
                        html: `${stopped ? '<p>Sync is off. This copy is now independent.</p>' : ''}<p>Keep <strong>${esc(node.name)}</strong> up to date on your other devices.</p><p>Only ${selection} ${node.kind === 'folder' ? 'are' : 'is'} included.</p>${problem ? `<p role="alert">${esc(problem)}</p>` : ''}`,
                        buttons: [
                            { label: 'Cancel', value: 'close' },
                            { label: 'Turn on sync', value: 'on', primary: true },
                        ],
                    });
                    if (answer.value !== 'on') break;
                    ui.busy('Turning on Sync…');
                    if (scope) {
                        scope.personalSync = true;
                        await saveScope(scope);
                    } else await enrollSelection(id);
                    if (!app.node(id)?.connection)
                        throw new SyncSetupError('saving-connections', new Error('binding-missing'));
                    problem = '';
                    openSyncLink = true;
                    continue;
                }
                if (openSyncLink && scope.member.grant.permission === 'owner') {
                    openSyncLink = false;
                    await syncLinkPanel(scope, ui);
                    if (!ui.open) break;
                }
                const owner = scope.member.grant.permission === 'owner',
                    current = app.node(app.state.currentId);
                const blocked = !!current && belongs(current, scope) && !viewer.canPublishConnection();
                const status = blocked ? 'attention' : (node.connection!.phase ?? 'waiting');
                const rotating = !!(await rotationRepository(scope).get());
                const label = {
                    current: 'Up to date',
                    working: 'Syncing changes…',
                    waiting: 'Waiting to sync',
                    offline: 'Offline',
                    attention: 'Sync needs attention',
                    ended: 'Sync stopped',
                }[status];
                const message = blocked
                    ? 'Your latest changes are not saved yet. Try again, or download a copy before closing.'
                    : status === 'offline'
                      ? db.isTemporaryStorage()
                          ? 'Your edits are waiting to sync. Keep this tab open or download a copy before closing.'
                          : 'Your changes are saved here and will sync when you are back online.'
                      : status === 'ended'
                        ? 'This connection is no longer available. Your document stays here.'
                        : status === 'attention'
                          ? 'Your changes are saved here. Try again to finish syncing.'
                          : owner
                            ? 'Use a Sync link to connect your other device.'
                            : 'Changes appear on your synced devices.';
                const rootName = app.node(scope.localRoot)?.name ?? node.name;
                const folder =
                    scope.kind === 'folder' && node.id !== scope.localRoot
                        ? `<p>This document syncs with the <strong>${esc(rootName)}</strong> folder. Sync links connect the whole folder.</p>`
                        : '';
                const answer = await ui.screen({
                    title: 'Sync',
                    html: `<p><strong>${esc(node.name)}</strong></p><p class="sync-status" data-state="${status}">${status === 'working' ? '<span class="su-spinner" aria-hidden="true"></span>' : icon(status === 'current' ? 'check' : status === 'ended' ? 'unlink' : status === 'attention' ? 'warning' : 'refresh')}<strong>${status === 'ended' ? 'Local copy · Sync ended' : label}</strong></p><p>${message}</p>${folder}${problem ? `<p role="alert">${esc(problem)}</p>` : ''}
                <nav class="sync-tools" aria-label="Sync settings">${owner && status !== 'ended' ? syncTool('Devices', 'access', 'refresh') : ''}</nav>
                ${scope.selection ? '<p>To connect a new device, open the original synced folder.</p>' : ''}
                <p class="sync-fineprint">${status === 'ended' ? 'Your local copies remain available.' : scope.kind === 'folder' ? 'Sync for this folder and its documents will stop after 30 days without opening the folder or a document inside. Local copies will remain.' : "Sync will stop if this document isn't opened for 30 days, but local copies will remain."}</p>`,
                    buttons: [
                        {
                            label: status === 'ended' ? 'Keep as local document' : 'Disconnect this device',
                            value: 'stop',
                            empty: true,
                            left: true,
                        },
                        ...(status === 'attention' || status === 'waiting'
                            ? [{ label: 'Try again', value: 'retry', primary: true }]
                            : owner && status !== 'ended' && !rotating && !scope.selection
                              ? [{ label: 'Sync link', value: 'link', primary: true }]
                              : []),
                    ],
                });
                problem = '';
                if (answer.value === 'close') break;
                if (answer.value === 'link') {
                    openSyncLink = true;
                    continue;
                }
                if (answer.value === 'access') await accessPanel(scope, ui);
                else if (answer.value === 'retry') {
                    ui.busy('Trying again…');
                    await viewer.flush();
                    if (scope.cleanup) await continueCleanup(scope, ui.signal);
                    else if (await rotationRepository(scope).get()) await continueRotation(scope);
                    else {
                        start(scope);
                        driverFor(scope)?.retry();
                    }
                } else if (answer.value === 'stop') {
                    const result = await ui.screen({
                        title: 'Disconnect this device?',
                        back: true,
                        html: `<p>${node.kind === 'folder' ? 'This folder and its documents stay on this device as independent copies.' : 'Your document stays on this device as an independent copy.'} Other devices and active Live links remain connected. This copy will no longer send or receive changes.</p>${node.connection!.liveShared ? '<p>If you also want to stop sharing, disable the Live links first.</p>' : ''}`,
                        buttons: [{ label: 'Disconnect', value: 'stop', primary: true }],
                    });
                    if (result.value === 'stop') {
                        ui.busy('Stopping sync…');
                        await stopHere(id);
                        stopped = true;
                    }
                }
            } catch (error) {
                const details =
                    error instanceof SyncSetupError ? { ...safeError(error), stage: error.stage } : safeError(error);
                if (!(error instanceof SyncSetupError)) syncDebug.record('setup.failed', details, true);
                problem = syncError(error);
                let unfinished: db.UnfinishedSetupSnapshot | undefined;
                if (details.code === 'orphan-replica') {
                    const node = app.node(id);
                    try {
                        if (node) {
                            const snapshot = await db.captureUnfinishedSetup([
                                id,
                                ...descendants(app.state.nodes, id).map((node) => node.id),
                            ]);
                            if (canRestartUnfinished(snapshot, id)) unfinished = snapshot;
                        }
                    } catch (error) {
                        syncDebug.record('setup.failed', { stage: 'checking-selection', ...safeError(error) }, true);
                    }
                }
                const result = await ui.screen({
                    title: 'Sync did not finish',
                    html: `<div class="sync-error" role="alert"><p><strong>${esc(problem)}</strong></p><p>This setup attempt did not delete local documents or comments.</p><p class="sync-fineprint">Error code: <code>${esc(details.code)}</code>${'stage' in details ? ` · Step: <code>${esc(details.stage)}</code>` : ''}</p></div>`,
                    buttons: [
                        ...(unfinished ? [{ label: 'Restart setup', value: 'restart', primary: true }] : []),
                        { label: 'Back to Sync', value: 'back', primary: !unfinished },
                    ],
                });
                if (result.value === 'close') break;
                if (result.value === 'restart' && unfinished) {
                    const answer = await ui.screen({
                        title: 'Restart unfinished Sync setup?',
                        back: true,
                        html: '<p>Start a new Sync connection for this folder and its documents.</p><p>Your current documents and comments stay unchanged. Unsent work from the previous setup also stays on this device.</p><p>Devices from the old setup need a new Sync link. Existing saved copies and links are not deleted.</p>',
                        buttons: [{ label: 'Restart setup', value: 'restart', primary: true }],
                    });
                    if (answer.value === 'restart') restart = unfinished;
                }
            }
        }
    } finally {
        ui.close();
    }
}
