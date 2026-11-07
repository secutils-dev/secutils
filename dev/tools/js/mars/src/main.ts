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

const LAST_DOC_KEY = 'lastDoc';
const DEFAULT_TITLE = document.title;

viewer.setHost({
    currentId: () => app.state.currentId,
    persist: app.persist,
    detach: app.detach,
    readingWidthChanged: app.setReadingWidth,
    openFile: share.receiveFile,
    openUrl: share.receiveUrl,
    temporary: db.isTemporaryStorage,
});

let lastDoc: string | null = null;
app.onChange(() => {
    const cur = app.current();
    document.title = cur ? `${cur.name} | MARS` : DEFAULT_TITLE;
    renderTemporary();
    if (app.state.currentId !== lastDoc) {
        lastDoc = app.state.currentId;
        if (!db.isTemporaryStorage()) db.setMeta(LAST_DOC_KEY, lastDoc).catch(() => {});
    }
});

function renderTemporary() {
    const temporary = db.isTemporaryStorage();
    $('temporarySession').hidden = !temporary;
    ($('readTemporaryBtn') as HTMLButtonElement).disabled = !app.current() || temporary;
    ($('saveToLibraryBtn') as HTMLButtonElement).disabled = !app.current() || app.state.temporaryLink !== 'ready';
    if (!temporary) return;
    $('sidebarTitle').textContent = 'Temporary documents';
    $('sidebar').setAttribute('aria-label', 'Temporary documents');
    $('tree').setAttribute('aria-label', 'Temporary documents');
    const status = app.state.storageError ? 'Storage unavailable · Temporary session' : 'Temporary session';
    if ($('temporaryStatus').textContent !== status) $('temporaryStatus').textContent = status;
    const message = app.state.temporaryLink === 'failed'
        ? 'MARS could not update the address bar. Download this document before closing. Nothing is saved in your library.'
        : app.state.temporaryLink === 'too-large'
            ? 'This document is too large for automatic URL recovery. Download it before closing. Nothing is saved in your library.'
            : 'Not saved in your library. The address bar restores the current document. Export other session documents before closing.';
    if ($('temporaryHelp').textContent !== message) $('temporaryHelp').textContent = message;
    $('emptyState').querySelector('.empty-state-sub')!.textContent = 'Open or drop a file, paste Markdown, or start writing. This session is temporary and is not saved in your library.';
    ($('returnToLibrary') as HTMLAnchorElement).href = `${location.pathname}?library=1`;
}
$('readTemporaryBtn').addEventListener('click', () => share.startTemporary(true).catch((error) => viewer.toast(error.message)));
$('saveToLibraryBtn').addEventListener('click', async () => {
    await viewer.flush();
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
document.documentElement.classList.remove('mars-boot');
const request = new URL(location.href);
const last = await db.getMeta<string>(LAST_DOC_KEY).catch(() => undefined);
// A failed ?url= load retains the previous document, just like the URL form.
if (!request.hash && request.searchParams.has('url') && last && app.node(last)) await app.openDoc(last);
// Generation guards make address-bar navigation safe while an initial fetch is pending.
window.addEventListener('hashchange', () => { share.openFromLocation(); });
if (db.isTemporaryStorage() && request.searchParams.get('new') === '1' && !request.hash && !request.searchParams.has('url')) {
    await app.createDoc();
    viewer.setViewMode('edit');
} else if (!(await share.openFromLocation(request.href))) {
    if (last && app.node(last)) await app.openDoc(last);
    else viewer.endLoading();
}
db.onRemoteChange(() => { app.load(); });
