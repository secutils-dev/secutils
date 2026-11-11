// MARS (Markdown Annotation & Review System) entry point.
//
// Owns the startup order: styles, diagnostics, the viewer host, storage, connections, and the
// first document. Feature modules own their behavior. This file connects them.
// Invariants:
// - `boot.ts` is the first import, so styles and icons exist before another module reads the DOM.
// - Storage opens before a document opens. A storage failure selects a temporary session.
// - A failure after storage opens becomes a toast. It never reaches the page loader in `index.html`.
import './boot.ts';
import * as viewer from './doc/viewer.js';
import * as app from './app.ts';
// Side-effect imports: each of these modules registers its own listeners when it loads.
import './sidebar/sidebar.ts';
import './doc/changes.ts';
import './export/dialog.ts';
import * as share from './share/share.ts';
import * as io from './io/io.ts';
import * as db from './store/db.ts';
import { $ } from './ui/ui.ts';
import * as sync from './sync/index.ts';
import { installTooltips } from './ui/feedback.ts';
import { installSyncDebug } from './sync/debug.ts';
import { installDeveloperTools } from './ui/developer.ts';
import { startAnalytics } from './platform/analytics.ts';

// Setup that needs no storage runs first, so it is also in place when a later step fails.
// `installDeveloperTools` parses the flags and removes their control parameters from the address bar.
// `installSyncDebug` records the `boot` trace event.
startAnalytics();
installDeveloperTools();
installSyncDebug();

/** `meta` key of the document that was open last. Startup opens it when the address names no document. */
const LAST_DOC_KEY = 'lastDoc';
// Shell texts, read before the first render changes them.
const DEFAULT_TITLE = document.title;
const DEFAULT_EMPTY_HELP = $('emptyState').querySelector('.empty-state-sub')!.textContent;

// The viewer does not import `app`. It reaches the tree, the storage mode, and Sync through this
// host, so the host is set before a document opens.
viewer.setHost({
    currentId: () => app.state.currentId,
    persist: app.persist,
    detach: app.detach,
    readingWidthChanged: app.setReadingWidth,
    openFile: share.receiveFile,
    openUrl: share.receiveUrl,
    temporary: db.isTemporaryStorage,
    connection: () => {
        const n = app.current();
        return n?.connection
            ? {
                  permission: n.connection.permission,
                  label: sync.label(n),
                  participant: n.connection.participant ?? n.connection.member,
                  ownerParticipant: n.connection.ownerParticipant,
                  ended: n.connection.phase === 'ended',
              }
            : null;
    },
    // Automatic writes to a linked file stay off for a connected document and for a document with
    // `manualFileSave`. A remote edit must never reach the disk without an explicit Save.
    fileAutoSave: () => !app.current()?.connection && !app.current()?.manualFileSave,
    undoConnection: (redo) => (app.state.currentId ? sync.undo(app.state.currentId, redo) : Promise.resolve()),
    undoReview: (before, after, id) => (id ? sync.undoReview(id, before, after) : Promise.resolve()),
    connectionSaved: () => {
        if (app.state.currentId) void sync.refresh([app.state.currentId]).catch((error) => viewer.toast(error.message));
    },
});

let lastDoc: string | null = null;
// One listener keeps the page chrome in step with the tree state. It updates the tab title,
// the Live access badge, the session banner, and the stored last document.
app.onChange(() => {
    const cur = app.current();
    document.title = cur ? `${cur.name} | MARS` : DEFAULT_TITLE;
    const access = $('documentAccess'),
        permission = cur?.connection?.permission;
    access.hidden = !cur?.connection || cur.connection.kind !== 'live' || permission === 'owner';
    if (!access.hidden && permission) {
        const labels = { view: 'View only', comment: 'Can comment', edit: 'Can edit', owner: '' };
        const descriptions = {
            view: 'You can read this document and its comments.',
            comment: 'You can add comments, but cannot change the document.',
            edit: 'You can change the document and manage comments.',
            owner: '',
        };
        if (access.textContent !== labels[permission]) access.textContent = labels[permission];
        access.dataset.tooltip =
            cur!.connection!.phase === 'ended'
                ? 'The Live link ended. Keep this as a local copy to make it independent.'
                : descriptions[permission];
    }
    renderTemporary();
    // A temporary session never writes library preferences. The write is best-effort.
    if (app.state.currentId !== lastDoc) {
        lastDoc = app.state.currentId;
        if (!db.isTemporaryStorage()) db.setMeta(LAST_DOC_KEY, lastDoc).catch(() => {});
    }
});

/**
 * Renders the session banner, the sidebar labels, and the library buttons.
 * The banner shows in a temporary session and for a document whose connection ended.
 * It reads `app.state` and `db.isTemporaryStorage()`, and changes the DOM only.
 */
function renderTemporary() {
    const temporary = db.isTemporaryStorage();
    const live = app.current()?.connection?.kind === 'live';
    const ended = app.current()?.connection?.phase === 'ended';
    $('temporarySession').hidden = !temporary && !ended;
    ($('readTemporaryBtn') as HTMLButtonElement).disabled = !app.current() || temporary;
    ($('saveToLibraryBtn') as HTMLButtonElement).disabled =
        !app.current() || (!live && app.state.temporaryLink !== 'ready');
    $('saveToLibraryBtn').textContent =
        ended && live
            ? 'Save local copy to library'
            : live
              ? 'Save live document to library'
              : 'Save current to library';
    $('saveToLibraryBtn').hidden = !temporary;
    $('returnToLibrary').hidden = !temporary;
    if (!temporary) {
        $('sidebarTitle').textContent = 'Documents';
        $('sidebar').setAttribute('aria-label', 'Documents');
        $('tree').setAttribute('aria-label', 'Documents');
        if ($('emptyState').querySelector('.empty-state-sub')!.textContent !== DEFAULT_EMPTY_HELP)
            $('emptyState').querySelector('.empty-state-sub')!.textContent = DEFAULT_EMPTY_HELP;
        if (!ended) return;
    } else {
        $('sidebarTitle').textContent = 'Temporary documents';
        $('sidebar').setAttribute('aria-label', 'Temporary documents');
        $('tree').setAttribute('aria-label', 'Temporary documents');
    }
    const status = ended
        ? 'Local copy · connection ended'
        : live
          ? 'Temporary live session'
          : app.state.storageError
            ? 'Storage unavailable · Temporary session'
            : 'Temporary session';
    if ($('temporaryStatus').textContent !== status) $('temporaryStatus').textContent = status;
    const message = ended
        ? temporary
            ? 'The Live link is no longer available. This copy and any unsent changes are still here. Download it or save it to your library before closing. Reload cannot restore it.'
            : `This connection ended. Your saved copy and any unsent changes are still here. Use ${live ? 'Live document' : 'Sync'} to keep it as an independent local document.`
        : live
          ? 'Not saved in your library. Reload restores acknowledged live content, not unsent edits. Download pending work before closing.'
          : app.state.temporaryLink === 'failed'
            ? 'MARS could not update the address bar. Download this document before closing. Nothing is saved in your library.'
            : app.state.temporaryLink === 'too-large'
              ? 'This document is too large for automatic URL recovery. Download it before closing. Nothing is saved in your library.'
              : 'Not saved in your library. The address bar restores the current document. Export other session documents before closing.';
    if ($('temporaryHelp').textContent !== message) $('temporaryHelp').textContent = message;
    if (temporary)
        $('emptyState').querySelector('.empty-state-sub')!.textContent =
            'Open or drop a file, paste Markdown, or start writing. This session is temporary and is not saved in your library.';
    ($('returnToLibrary') as HTMLAnchorElement).href = `${location.pathname}?library=1`;
}
$('readTemporaryBtn').addEventListener('click', () =>
    share.startTemporary(true).catch((error) => viewer.toast(error.message)),
);
$('setSharedReadingWidth').addEventListener('click', () => {
    if (app.state.currentId)
        void sync
            .sharedWidth(app.state.currentId, viewer.exportReadingWidth())
            .catch((error) => viewer.toast(error.message));
});
// Save to library. A live document moves in one atomic promotion (`sync.saveLive`).
// Another temporary document travels in the fragment. The page reloads on the library
// address with the same fragment, without `t=1`, and receives it as a full link.
$('saveToLibraryBtn').addEventListener('click', async () => {
    await viewer.flush();
    if (app.current()?.connection?.kind === 'live') {
        await sync.saveLive(app.state.currentId!);
        return;
    }
    // The fragment holds the current document only in the `ready` state.
    if (app.state.temporaryLink !== 'ready') {
        viewer.toast('Download the document to keep it.');
        return;
    }
    const target = new URL(location.href),
        params = new URLSearchParams(target.hash.slice(1));
    params.delete('t');
    target.hash = params.toString();
    target.search = '?library=1';
    location.assign(target.href);
});

// File and folder pickers of the sidebar. Reset the value so that the same selection fires `change` again.
for (const id of ['importInput', 'importFolderInput']) {
    const input = $<HTMLInputElement>(id);
    input.addEventListener('change', () => {
        const files = [...(input.files ?? [])];
        input.value = '';
        if (files.length) io.importFiles(files);
    });
}

// Starts a save of pending edits. The browser does not wait for it.
window.addEventListener('pagehide', () => {
    viewer.flush();
});

// Open storage and load the tree. Any failure here (denied storage, a full quota, a blocked
// upgrade) selects the memory backend, so the app still starts.
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
// From here on a failure must not reach the page loader: it replaces the whole page with an error,
// and the reader then has no route to the library or to an export.
const startupError = (action: string) => (error: unknown) => {
    viewer.toast(`${action}: ${(error as Error)?.message ?? error}. Your documents are still in this browser.`);
};
// The connection manager is a lazy module. It loads only when the library holds connected
// content, and it starts before the first document opens.
if (app.state.nodes.some((n) => n.connection)) await sync.initialize().catch(startupError('Sync could not start'));
// The shell hides the body with `html.mars-boot`. Reveal it when the styles, the tree, and the
// connection state are ready.
document.documentElement.classList.remove('mars-boot');
installTooltips();
// The address at startup. The steps below read this copy, because `openDoc` rewrites `location`.
const request = new URL(location.href);
const last = await db.getMeta<string>(LAST_DOC_KEY).catch(() => undefined);
// Generation guards make address-bar navigation safe while an initial fetch is pending.
// Connection links (`#sync=`, `#invite=`, `#live=`) go to Sync. Every other fragment goes to `share`.
// The listener exists before the first open, so a navigation during that open is not lost.
window.addEventListener('hashchange', async () => {
    try {
        if (!(await sync.receiveLocation(location.href))) await share.openFromLocation();
    } catch (error) {
        startupError('Could not open this link')(error);
    }
});
// The first document comes from the address: a connection link, a share link, `#doc=`, or
// `?url=`. Without one, the document that was open last opens, or the empty state shows.
try {
    // A failed ?url= load retains the previous document, just like the URL form.
    if (!request.hash && request.searchParams.has('url') && last && app.node(last)) await app.openDoc(last);
    // `?temporary=1&new=1` comes from New temporary document: start with an empty document in Source mode.
    if (
        db.isTemporaryStorage() &&
        request.searchParams.get('new') === '1' &&
        !request.hash &&
        !request.searchParams.has('url')
    ) {
        await app.createDoc();
        viewer.setViewMode('edit');
    } else if (
        !(sync.isConnectionLocation(request.href)
            ? await sync.receiveLocation(request.href)
            : await share.openFromLocation(request.href))
    ) {
        if (last && app.node(last)) await app.openDoc(last);
        else viewer.endLoading();
    }
} catch (error) {
    startupError('Could not open the document')(error);
    viewer.endLoading();
}
// Another tab changed the library. Reload the tree, adopt the change in the open document, and
// refresh connected sessions. The persistence queue keeps this work apart from a save of this tab.
// Registered last: the handler needs the loaded tree. An earlier broadcast is not replayed.
db.onRemoteChange((ids) => {
    void app
        .serializePersistence(async () => {
            await app.load();
            await app.adoptRemoteChange(ids);
            if (app.state.nodes.some((node) => node.connection)) await sync.refresh(ids);
        })
        .catch((error) => viewer.toast(`Could not refresh this library: ${error.message}. Local edits are retained.`));
});
