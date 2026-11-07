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
    openFile: share.receiveFile,
    openUrl: share.receiveUrl,
});

let lastDoc: string | null = null;
app.onChange(() => {
    const cur = app.current();
    document.title = cur ? `${cur.name} | MARS` : DEFAULT_TITLE;
    if (app.state.currentId !== lastDoc) {
        lastDoc = app.state.currentId;
        db.setMeta(LAST_DOC_KEY, lastDoc).catch(() => {});
    }
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
    viewer.toast(`Browser storage is unavailable, so reviews won't be kept: ${(err as Error)?.message ?? err}`);
}
document.documentElement.classList.remove('mars-boot');
const request = new URL(location.href);
const last = await db.getMeta<string>(LAST_DOC_KEY).catch(() => undefined);
// A failed ?url= load retains the previous document, just like the URL form.
if (!request.hash && request.searchParams.has('url') && last && app.node(last)) await app.openDoc(last);
// Generation guards make address-bar navigation safe while an initial fetch is pending.
window.addEventListener('hashchange', () => { share.openFromLocation(); });
if (!(await share.openFromLocation(request.href))) {
    if (last && app.node(last)) await app.openDoc(last);
    else viewer.endLoading();
}
db.onRemoteChange(() => { app.load(); });
