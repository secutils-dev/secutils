// MARS (Markdown Annotation & Review System) entry point.
import './boot.ts';
import * as viewer from './doc/viewer.js';
import * as app from './app.ts';
import './sidebar/sidebar.ts';
import './doc/changes.ts';
import './export/dialog.ts';
import * as share from './share/share.ts';
import * as io from './io/io.ts';
import * as db from './store/db.ts';
import { $ } from './ui/ui.ts';
import * as sync from './sync/index.ts';
import { installTooltips } from './ui/feedback.ts';

const LAST_DOC_KEY = 'lastDoc';
const DEFAULT_TITLE = document.title;
const DEFAULT_EMPTY_HELP = $('emptyState').querySelector('.empty-state-sub')!.textContent;

viewer.setHost({
    currentId: () => app.state.currentId,
    persist: app.persist,
    detach: app.detach,
    readingWidthChanged: app.setReadingWidth,
    openFile: share.receiveFile,
    openUrl: share.receiveUrl,
    temporary: db.isTemporaryStorage,
    connection: () => { const n = app.current(); return n?.connection ? { permission: n.connection.permission, label: sync.label(n), participant: n.connection.participant ?? n.connection.member, ownerParticipant: n.connection.ownerParticipant, ended: n.connection.phase === 'ended' } : null; },
    fileAutoSave: () => !app.current()?.connection && !app.current()?.manualFileSave,
    undoConnection: (redo) => app.state.currentId ? sync.undo(app.state.currentId, redo) : Promise.resolve(),
    undoReview: (before, after, id) => id ? sync.undoReview(id, before, after) : Promise.resolve(),
    connectionSaved: () => { if (app.state.currentId) void sync.refresh([app.state.currentId]).catch((error) => viewer.toast(error.message)); },
});

let lastDoc: string | null = null;
app.onChange(() => {
    const cur = app.current();
    document.title = cur ? `${cur.name} | MARS` : DEFAULT_TITLE;
    const access = $('documentAccess'), permission = cur?.connection?.permission;
    access.hidden = !cur?.connection || cur.connection.kind !== 'live' || permission === 'owner';
    if (!access.hidden && permission) {
        const labels = { view: 'View only', comment: 'Can comment', edit: 'Can edit', owner: '' };
        const descriptions = { view: 'You can read this document and its comments.', comment: 'You can add comments, but cannot change the document.', edit: 'You can change the document and manage comments.', owner: '' };
        if (access.textContent !== labels[permission]) access.textContent = labels[permission];
        access.dataset.tooltip = cur!.connection!.phase === 'ended' ? 'The Live link ended. Keep this as a local copy to make it independent.' : descriptions[permission];
    }
    renderTemporary();
    if (app.state.currentId !== lastDoc) {
        lastDoc = app.state.currentId;
        if (!db.isTemporaryStorage()) db.setMeta(LAST_DOC_KEY, lastDoc).catch(() => {});
    }
});

function renderTemporary() {
    const temporary = db.isTemporaryStorage();
    const live = app.current()?.connection?.kind === 'live';
    const ended = app.current()?.connection?.phase === 'ended';
    $('temporarySession').hidden = !temporary && !ended;
    ($('readTemporaryBtn') as HTMLButtonElement).disabled = !app.current() || temporary;
    ($('saveToLibraryBtn') as HTMLButtonElement).disabled = !app.current() || (!live && app.state.temporaryLink !== 'ready');
    $('saveToLibraryBtn').textContent = ended && live ? 'Save local copy to library' : live ? 'Save live document to library' : 'Save current to library';
    $('saveToLibraryBtn').hidden = !temporary; $('returnToLibrary').hidden = !temporary;
    if (!temporary) {
        $('sidebarTitle').textContent = 'Documents'; $('sidebar').setAttribute('aria-label', 'Documents'); $('tree').setAttribute('aria-label', 'Documents');
        if ($('emptyState').querySelector('.empty-state-sub')!.textContent !== DEFAULT_EMPTY_HELP) $('emptyState').querySelector('.empty-state-sub')!.textContent = DEFAULT_EMPTY_HELP;
        if (!ended) return;
    } else { $('sidebarTitle').textContent = 'Temporary documents'; $('sidebar').setAttribute('aria-label', 'Temporary documents'); $('tree').setAttribute('aria-label', 'Temporary documents'); }
    const status = ended ? 'Local copy · connection ended' : live ? 'Temporary live session' : app.state.storageError ? 'Storage unavailable · Temporary session' : 'Temporary session';
    if ($('temporaryStatus').textContent !== status) $('temporaryStatus').textContent = status;
    const message = ended ? temporary
        ? 'The Live link is no longer available. This copy and any unsent changes are still here. Download it or save it to your library before closing. Reload cannot restore it.'
        : `This connection ended. Your saved copy and any unsent changes are still here. Use ${live ? 'Live document' : 'Sync'} to keep it as an independent local document.`
        : live ? 'Not saved in your library. Reload restores acknowledged live content, not unsent edits. Download pending work before closing.' : app.state.temporaryLink === 'failed'
        ? 'MARS could not update the address bar. Download this document before closing. Nothing is saved in your library.'
        : app.state.temporaryLink === 'too-large'
            ? 'This document is too large for automatic URL recovery. Download it before closing. Nothing is saved in your library.'
            : 'Not saved in your library. The address bar restores the current document. Export other session documents before closing.';
    if ($('temporaryHelp').textContent !== message) $('temporaryHelp').textContent = message;
    if (temporary) $('emptyState').querySelector('.empty-state-sub')!.textContent = 'Open or drop a file, paste Markdown, or start writing. This session is temporary and is not saved in your library.';
    ($('returnToLibrary') as HTMLAnchorElement).href = `${location.pathname}?library=1`;
}
$('readTemporaryBtn').addEventListener('click', () => share.startTemporary(true).catch((error) => viewer.toast(error.message)));
$('setSharedReadingWidth').addEventListener('click', () => { if (app.state.currentId) void sync.sharedWidth(app.state.currentId, viewer.exportReadingWidth()).catch((error) => viewer.toast(error.message)); });
$('saveToLibraryBtn').addEventListener('click', async () => {
    await viewer.flush();
    if (app.current()?.connection?.kind === 'live') { await sync.saveLive(app.state.currentId!); return; }
    if (app.state.temporaryLink !== 'ready') { viewer.toast('Download the document to keep it.'); return; }
    const target = new URL(location.href), params = new URLSearchParams(target.hash.slice(1));
    params.delete('t'); target.hash = params.toString(); target.search = '?library=1';
    location.assign(target.href);
});

for (const id of ['importInput', 'importFolderInput']) {
    const input = $<HTMLInputElement>(id);
    input.addEventListener('change', () => {
        const files = [...(input.files ?? [])];
        input.value = '';
        if (files.length) io.importFiles(files);
    });
}

window.addEventListener('pagehide', () => { viewer.flush(); });

try {
    db.persistStorage();
    await app.load();
} catch (err) {
    db.useTemporaryStorage();
    app.state.storageError = (err as Error)?.message ?? String(err);
    await app.load();
    viewer.toast('Browser storage is unavailable. Opened a temporary session instead.');
}
renderTemporary();
if (await db.getMeta<boolean>('sync:safety')) viewer.enableConnectionSafety();
if (app.state.nodes.some((n) => n.connection)) await sync.initialize();
document.documentElement.classList.remove('mars-boot');
installTooltips();
const request = new URL(location.href);
const last = await db.getMeta<string>(LAST_DOC_KEY).catch(() => undefined);
// A failed ?url= load retains the previous document, just like the URL form.
if (!request.hash && request.searchParams.has('url') && last && app.node(last)) await app.openDoc(last);
// Generation guards make address-bar navigation safe while an initial fetch is pending.
window.addEventListener('hashchange', async () => { if (!await sync.receiveLocation(location.href)) await share.openFromLocation(); });
if (db.isTemporaryStorage() && request.searchParams.get('new') === '1' && !request.hash && !request.searchParams.has('url')) {
    await app.createDoc();
    viewer.setViewMode('edit');
} else if (!(sync.isConnectionLocation(request.href) ? await sync.receiveLocation(request.href) : await share.openFromLocation(request.href))) {
    if (last && app.node(last)) await app.openDoc(last);
    else viewer.endLoading();
}
db.onRemoteChange((ids) => { void app.serializePersistence(async () => {
    if (await db.getMeta<boolean>('sync:safety')) viewer.enableConnectionSafety();
    await app.load();
    if (app.state.nodes.some((node) => node.connection)) await sync.refresh(ids);
}).catch((error) => viewer.toast(`Could not refresh this library: ${error.message}. Local edits are retained.`)); });
