// The Sync dialog for one tree node: setup, status, retry, device management, and disconnection.
// This module also routes a node to its dialog (Sync, Live links, or Live document) and maps fixed
// error codes to user-visible sentences.
// It owns screens and their flow only. Enrollment, pairing, access changes, and Live links live in the
// sibling modules, and each loop pass reads their current state.
// Invariants: document names, error sentences, and codes reach dialog markup only through `esc()`.
// Raw error text never reaches a screen: the reader sees a mapped sentence and an allowlisted code.
import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { ancestors, descendants } from '../../tree/model.ts';
import { esc } from '../../ui/ui.ts';
import { LIMITS, SyncError } from '../protocol.ts';
import { SyncDialog, type FlowScreen } from '../dialog.ts';
import { icon, type IconName } from '../../ui/icons.ts';
import { syncDebug, safeError } from '../debug.ts';
import { rootFor, sessions, belongs, saveScope, rotationRepository, driverFor, rotations, scopeId } from './state.ts';
import { initialize, start } from './engine.ts';
import { SyncSetupError, enrollSelection, canRestartUnfinished } from './enrollment.ts';
import { syncLinkPanel } from './pairing.ts';
import { liveDocument, liveLinks, continueCleanup } from './live-links.ts';
import { accessPanel, continueRotation, stopHere } from './access.ts';
import { pendingCreations, resumeCreations } from './creation.ts';
import { insideSyncedFolder } from '../inheritance.ts';
import { label as itemLabel } from '../index.ts';

/** Opens the Sync dialog for node `id`: the setup offer for a local node, or the status of a connection. */
export const enroll = (id: string) => syncDialog(id);
/**
 * Opens the dialog that matches the connection of node `id`: **Live document** for a received Live link,
 * **Live links** for a document that the owner shares through links, and the Sync dialog otherwise.
 */
export async function details(id: string) {
    await initialize();
    const node = app.node(id);
    if (node?.connection?.kind === 'live') return liveDocument(id);
    return node?.connection?.liveShared ? liveLinks(id) : syncDialog(id);
}
/**
 * Maps a failure to one user-visible sentence.
 *
 * Only allowlisted codes reach the lookup. Unknown failures return a fixed generic sentence,
 * so an untrusted service error cannot expose raw text or select an inherited object property.
 * The result is plain text: the caller escapes it for markup.
 */
export const syncError = (error: unknown) =>
    ({
        'document-limit': `This Sync collection reached its limit of ${LIMITS.documents.toLocaleString('en-US')} documents and folders. The root and nested folders count too. Your files are saved locally. Disconnect this device. Split the content into smaller folders. Then turn on Sync for each folder separately. Retrying without changing the selection cannot increase the limit.`,
        'membership-or-document-limit': `The service limit for this connection was reached. A Sync collection supports ${LIMITS.documents.toLocaleString('en-US')} documents and folders. Your files are saved locally. For an imported folder, disconnect this device and sync smaller folders separately. For device access, disconnect an unused device.`,
        'service-unavailable':
            'The Sync service is unavailable right now. Your documents are still here. Try again later.',
        'binding-missing':
            'This tab lost the saved connection during setup. MARS has not reset your library. Try again.',
        'orphan-replica':
            'An earlier setup left saved connection data without a connected item. Your saved documents stay unchanged.',
        'already-connected':
            'Some selected items are already connected. Open Sync for those items before connecting the whole folder.',
        'selection-too-large': `This selection exceeds the limit of ${LIMITS.documents.toLocaleString('en-US')} documents and folders, including the root and nested folders. Your files are saved locally. Turn on Sync for smaller folders separately.`,
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
    })[safeError(typeof error === 'string' ? { message: error } : error).code] ??
    'Could not finish. Your document is still here. Try again.';
/**
 * Markup for an action button inside a dialog body. `SyncDialog` resolves a click with `action`.
 * The arguments are not escaped: pass literals only.
 */
const syncTool = (label: string, action: string, glyph: IconName) =>
    `<button type="button" class="btn" data-sync-action="${action}">${icon(glyph)}<span>${label}</span></button>`;
/**
 * Markup for a pairing confirmation code. `phrase` holds groups separated by single spaces, as
 * `pairingPhrase` returns them. Each group is escaped.
 */
export const confirmationCode = (phrase: string) =>
    `<div class="sync-code" role="group" aria-label="Confirmation code">${phrase
        .split(' ')
        .map((part) => `<span>${esc(part)}</span>`)
        .join(' ')}</div>`;

/** Keeps a status screen current until an action or dismissal. Subscription cleanup precedes the next flow screen. */
async function liveScreen(ui: SyncDialog, read: () => FlowScreen | undefined) {
    const screen = read();
    if (!screen) {
        ui.close();
        return { value: 'close', text: '' };
    }
    const answer = ui.screen(screen);
    const stop = app.onChange(() => {
        const next = read();
        if (next) ui.update(next);
        else ui.close();
    });
    try {
        return await answer;
    } finally {
        stop();
    }
}
/**
 * Runs the Sync dialog for node `id` until the reader closes it.
 *
 * Each loop pass reads the current state and shows one screen:
 * - a received Live document hands over to `liveDocument`;
 * - a local node, or an owner document that only backs Live links, gets the offer to turn on Sync;
 * - a personal Sync connection shows its status with the device list, Sync link, retry, and disconnect actions.
 *
 * A failure shows the error screen with a mapped sentence, a fixed code, and the setup step when known.
 * An `orphan-replica` failure also offers a restart when the preserved snapshot qualifies.
 */
async function syncDialog(id: string) {
    await initialize();
    const selected = app.node(id);
    const own = selected ? rootFor(selected) : undefined;
    const inherited =
        selected && !selected.connection
            ? ancestors(app.state.nodes, selected.id)
                  .reverse()
                  .map(rootFor)
                  .find((scope) => scope?.kind === 'folder' && !scope.selection)
            : undefined;
    const folderScope =
        own?.kind === 'folder' &&
        !own.selection &&
        (selected?.id === own.localRoot ||
            (selected?.connection?.phase !== 'ended' && insideSyncedFolder(app.state.nodes, id)))
            ? own
            : inherited;
    const itemId = id;
    const managedFolder = folderScope && folderScope.localRoot !== itemId ? folderScope.localRoot : undefined;
    const ui = new SyncDialog();
    if (managedFolder) ui.element.dataset.inheritedContext = '';
    let showingItem = !!managedFolder;
    // `openSyncLink` sends the owner to the Sync link screen on the next pass. `problem` is the sentence
    // for the next screen. `stopped` records that this dialog disconnected the device.
    let openSyncLink = false,
        problem = '',
        stopped = false;
    // The snapshot that the reader chose to restart from on the error screen.
    let restart: db.UnfinishedSetupSnapshot | undefined;
    try {
        while (ui.open) {
            try {
                if (managedFolder && showingItem) {
                    const answer = await liveScreen(ui, () => {
                        const item = app.node(itemId),
                            folder = app.node(managedFolder);
                        if (!item || !folder) return;
                        const connected = !!folder.connection && folder.connection.phase !== 'ended';
                        const state = connected
                            ? (item.syncPending?.phase ?? item.connection?.phase ?? 'waiting')
                            : 'ended';
                        const status = connected
                            ? itemLabel(item) || 'Saved locally · Sync enrollment pending'
                            : 'Local copy · folder Sync ended';
                        const path = [...ancestors(app.state.nodes, folder.id), folder]
                            .map((node) => node.name)
                            .join(' / ');
                        const failure = item.syncPending?.error;
                        return {
                            title: 'Sync',
                            html: `<p data-sync-field="name"><strong>${esc(item.name)}</strong></p><p data-sync-field="status" class="sync-status" data-state="${state}" role="status" aria-atomic="true">${state === 'working' ? '<span class="su-spinner" aria-hidden="true"></span>' : icon(state === 'current' ? 'check' : state === 'ended' ? 'unlink' : state === 'attention' ? 'warning' : 'refresh')}<strong>${esc(status)}</strong></p>
                            <p data-sync-field="folder">${connected ? 'Sync inherited from' : 'Folder Sync is disconnected for'} <strong>${esc(path)}</strong>.</p>
                            <p data-sync-field="error" class="sync-error" role="alert"${failure ? '' : ' hidden'}>${failure ? esc(syncError(new SyncError(failure))) : ''}</p>
                            <p class="sync-fineprint">Device settings and Sync links apply to that folder and all its contents.</p>`,
                            buttons: [{ label: 'Manage folder Sync', value: 'manage', primary: true }],
                        };
                    });
                    if (answer.value === 'close') break;
                    if (answer.value === 'manage') {
                        showingItem = false;
                        id = managedFolder;
                    }
                    continue;
                }
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
                // Personal Sync, as opposed to a document that only backs Live links. A non-owner connection
                // and a folder are always personal. A scope without the flag counts as personal unless the
                // document has Live links and no paired device.
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
                    // A Live-only scope already has a collection. Turning on Sync only sets its flag.
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
                const answer = await liveScreen(ui, () => {
                    const node = app.node(id);
                    if (!node?.connection || !belongs(node, scope)) return;
                    const owner = scope.member.grant.permission === 'owner',
                        current = app.node(app.state.currentId);
                    const enrolling = pendingCreations(scope);
                    const enrollmentError = enrolling.find((item) => item.syncPending?.error)?.syncPending?.error;
                    // The open document cannot publish its latest edits yet. Show `attention`, whatever the
                    // transport reports.
                    const blocked = !!current && belongs(current, scope) && !viewer.canPublishConnection();
                    const status =
                        node.connection!.phase === 'ended'
                            ? 'ended'
                            : blocked || enrolling.some((item) => item.syncPending?.phase === 'attention')
                              ? 'attention'
                              : enrolling.length
                                ? 'waiting'
                                : (node.connection!.phase ?? 'waiting');
                    const rotating = rotations.has(scopeId(scope));
                    if (status === 'current') problem = '';
                    const transportError = driverFor(scope)?.lastFailure;
                    const failure = enrollmentError
                        ? syncError(new SyncError(enrollmentError))
                        : status === 'attention' && transportError
                          ? syncError(transportError)
                          : problem;
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
                            ? `<p data-sync-field="folder">This document syncs with the <strong>${esc(rootName)}</strong> folder. Sync links connect the whole folder.</p>`
                            : '';
                    const scopeDescription =
                        scope.kind === 'folder' && node.id === scope.localRoot
                            ? '<p class="sync-fineprint">Folder settings apply to this folder and all its contents.</p>'
                            : '';
                    return {
                        title: 'Sync',
                        back: !!managedFolder,
                        html: `<p data-sync-field="name"><strong>${esc(node.name)}</strong></p>${scopeDescription}<p data-sync-field="status" class="sync-status" data-state="${status}" role="status" aria-atomic="true">${status === 'working' ? '<span class="su-spinner" aria-hidden="true"></span>' : icon(status === 'current' ? 'check' : status === 'ended' ? 'unlink' : status === 'attention' ? 'warning' : 'refresh')}<strong>${status === 'ended' ? 'Local copy · Sync ended' : label}</strong></p><p data-sync-field="error" class="sync-error" role="alert"${failure ? '' : ' hidden'}>${esc(failure)}</p><p data-sync-field="message"${failure ? ' hidden' : ''}>${esc(message)}</p>${folder}
                <p data-sync-field="pending"${enrolling.length ? '' : ' hidden'}>${enrolling.length} ${enrolling.length === 1 ? 'item is' : 'items are'} saved locally and waiting to join Sync.</p>
                <nav data-sync-field="settings" class="sync-tools" aria-label="Sync settings"${owner && status !== 'ended' ? '' : ' hidden'}>${syncTool('Devices', 'access', 'refresh')}</nav>
                ${scope.selection ? '<p>To connect a new device, open the original synced folder.</p>' : ''}
                <p data-sync-field="expiry" class="sync-fineprint">${status === 'ended' ? 'Your local copies remain available.' : scope.kind === 'folder' ? 'Sync for this folder and its documents will stop after 30 days without opening the folder or a document inside. Local copies will remain.' : "Sync will stop if this document isn't opened for 30 days, but local copies will remain."}</p>`,
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
                    };
                });
                problem = '';
                if (answer.value === 'back' && managedFolder) {
                    showingItem = true;
                    id = itemId;
                    continue;
                }
                if (answer.value === 'close') break;
                if (answer.value === 'link') {
                    openSyncLink = true;
                    continue;
                }
                if (answer.value === 'access') await accessPanel(scope, ui);
                else if (answer.value === 'retry') {
                    ui.busy('Trying again…');
                    await viewer.flush();
                    await resumeCreations(true);
                    // Resume in this order: a remote cleanup, then a saved key rotation, then the transport.
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
                // `safeError` reduces the failure to allowlisted fields. A `SyncSetupError` recorded its
                // own trace event, so only other failures are recorded here.
                const details =
                    error instanceof SyncSetupError ? { ...safeError(error), stage: error.stage } : safeError(error);
                if (!(error instanceof SyncSetupError)) syncDebug.record('setup.failed', details, true);
                problem = syncError(error);
                let unfinished: db.UnfinishedSetupSnapshot | undefined;
                // Saved connection data exists without a connected node. Offer a restart only when
                // `canRestartUnfinished` accepts the snapshot of this subtree.
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
