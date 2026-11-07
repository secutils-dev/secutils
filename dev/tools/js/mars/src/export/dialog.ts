import * as app from '../app.ts';
import * as viewer from '../doc/viewer.js';
import * as db from '../store/db.ts';
import { $, closeMenu, copyRich, download, fileSafe } from '../ui/ui.ts';
import { htmlDocument } from './html.ts';
import { prepareFrame, printDocument, type PreparedFrame } from './pdf.ts';
import { commitSettings, loadSettings, validWidth, type ExportFormat, type ExportSettings } from './settings.ts';
import { renderKey, snapshot, type ExportSnapshot } from './snapshot.ts';

const dialog = $<HTMLDialogElement>('exportDialog');
const setTitle = (name: string) => { $('exportDialogTitle').textContent = `Export “${name}”`; };
const htmlTab = $<HTMLButtonElement>('exportHtmlTab'), pdfTab = $<HTMLButtonElement>('exportPdfTab');
const width = $<HTMLSelectElement>('exportWidth'), customWidth = $<HTMLInputElement>('exportCustomWidth');
const paper = $<HTMLSelectElement>('exportPaper'), orientation = $<HTMLSelectElement>('exportOrientation');
const copy = $<HTMLButtonElement>('exportCopyHtml'), htmlDownload = $<HTMLButtonElement>('exportDownloadHtml');
const print = $<HTMLButtonElement>('exportPrint'), markdown = $<HTMLButtonElement>('downloadMdBtn');
const status = $('exportStatus'), retry = $('exportRetry');
const checkboxes = { toc: $<HTMLInputElement>('exportToc'), find: $<HTMLInputElement>('exportFind'), pdf: $<HTMLInputElement>('exportPdfAction'), source: $<HTMLInputElement>('exportSource') };
const containers = { html: $('exportHtmlPreview'), pdf: $('exportPdfPreview') };
interface Output { key: string; html: string; prepared: PreparedFrame; ready: boolean }
let outputs: Partial<Record<ExportFormat, Output>> = {};
let value: ExportSnapshot | null = null;
let draft: ExportSettings;
let format: ExportFormat = 'html';
let generation = 0, session = 0;
let renderAbort: AbortController | null = null;
let rendered: { key: string; promise: Promise<string> } | null = null;
let lastCustom = 800;
let invalidWidth = false;
let actionBusy = false;
let requestedId: string | null = null;

const disposeOutputs = () => { for (const out of Object.values(outputs)) out.prepared.dispose(); outputs = {}; };
const currentOutput = () => outputs[format];
const settingsKey = () => format === 'html' ? JSON.stringify(draft) : JSON.stringify(draft.pdf);
const setStatus = (message = '') => { status.textContent = message; $('exportStatusBar').hidden = !message; };
const previewReady = () => { setStatus(); containers[format].removeAttribute('aria-busy'); actions(true); };
function actions(ready = false) {
    ready = ready && !!currentOutput()?.ready;
    copy.disabled = htmlDownload.disabled = !ready || actionBusy || invalidWidth;
    print.disabled = !ready || actionBusy;
    markdown.disabled = !value || actionBusy;
}
function syncFields() {
    width.value = draft.html.readingWidth === 'full' ? 'full' : [800, 1100].includes(draft.html.readingWidth) ? String(draft.html.readingWidth) : 'custom';
    if (typeof draft.html.readingWidth === 'number') lastCustom = draft.html.readingWidth;
    customWidth.value = String(lastCustom);
    $('exportCustomWrap').hidden = width.value !== 'custom';
    for (const key of Object.keys(checkboxes) as (keyof typeof checkboxes)[]) checkboxes[key].checked = draft.html[key];
    paper.value = draft.pdf.paper; orientation.value = draft.pdf.orientation;
    invalidWidth = false; customWidth.setAttribute('aria-invalid', 'false'); $('exportWidthError').hidden = true;
}
function syncFormat() {
    for (const [name, tab] of [['html', htmlTab], ['pdf', pdfTab]] as const) {
        const selected = name === format;
        tab.setAttribute('aria-selected', String(selected)); tab.tabIndex = selected ? 0 : -1;
        tab.classList.toggle('active', selected);
        $(`export${name === 'html' ? 'Html' : 'Pdf'}Panel`).hidden = !selected;
        containers[name].hidden = !selected;
    }
    copy.hidden = htmlDownload.hidden = format !== 'html'; print.hidden = format !== 'pdf';
}
async function prepare() {
    const operationGeneration = ++generation;
    actions(); retry.hidden = true;
    if (!value || (format === 'html' && invalidWidth)) { setStatus(); return; }
    const activeFormat = format, captured = value, key = settingsKey();
    setStatus('Preparing preview…'); containers[activeFormat].setAttribute('aria-busy', 'true');
    const saved = outputs[activeFormat];
    if (saved?.key === key) {
        try { await saved.prepared.ready; if (operationGeneration === generation && dialog.open) previewReady(); }
        catch (error) { if (operationGeneration === generation) failure(error); }
        return;
    }
    saved?.prepared.dispose(); delete outputs[activeFormat];
    try {
        const bodyKey = renderKey(captured);
        if (!rendered || rendered.key !== bodyKey) {
            renderAbort?.abort(); renderAbort = new AbortController();
            rendered = { key: bodyKey, promise: viewer.renderExportBody(captured.body, renderAbort.signal) };
        }
        const content = await rendered.promise;
        if (operationGeneration !== generation || !dialog.open || captured !== value) return;
        const operation = crypto.randomUUID();
        const html = activeFormat === 'html' ? htmlDocument(captured, content, draft, operation) : printDocument(content, captured.name, draft.pdf, operation);
        const prepared = prepareFrame(containers[activeFormat], html, operation, activeFormat, () => {
            if (!dialog.open || captured !== value) return;
            commitSettings(draft); viewer.toast('Print requested. Choose Save as PDF in the browser dialog.');
        }, () => {
            // Safari does not restore the parent control after an opaque frame prints.
            setTimeout(() => {
                if (dialog.open && captured === value && format === activeFormat && activeFormat === 'pdf') print.focus();
            }, 0);
        });
        const output = { key, html, prepared, ready: false };
        outputs[activeFormat] = output;
        await prepared.ready;
        output.ready = true;
        if (operationGeneration !== generation || !dialog.open) return;
        previewReady();
    } catch (error) { if (operationGeneration === generation && dialog.open) failure(error); }
}
function failure(error: unknown) {
    setStatus(`Could not prepare ${format.toUpperCase()} preview: ${(error as Error).message ?? error}`);
    retry.hidden = false; containers[format].removeAttribute('aria-busy'); actions();
}
function switchFormat(next: ExportFormat) { format = next; syncFormat(); prepare(); }
htmlTab.addEventListener('click', () => switchFormat('html'));
pdfTab.addEventListener('click', () => switchFormat('pdf'));
$('exportTabs').addEventListener('keydown', (event) => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === 'Home' ? 'html' : event.key === 'End' ? 'pdf' : format === 'html' ? 'pdf' : 'html';
    switchFormat(next); (next === 'html' ? htmlTab : pdfTab).focus();
});
width.addEventListener('change', () => {
    $('exportCustomWrap').hidden = width.value !== 'custom';
    if (width.value === 'custom') customWidth.value = String(lastCustom);
    draft.html.readingWidth = width.value === 'custom' ? lastCustom : width.value === 'full' ? 'full' : Number(width.value);
    invalidWidth = false; $('exportWidthError').hidden = true; customWidth.setAttribute('aria-invalid', 'false');
    if (width.value === 'custom') customWidth.focus();
    prepare();
});
customWidth.addEventListener('input', () => {
    const next = Number(customWidth.value);
    invalidWidth = !customWidth.value || !validWidth(next);
    $('exportWidthError').hidden = !invalidWidth; customWidth.setAttribute('aria-invalid', String(invalidWidth));
    if (!invalidWidth) { lastCustom = next; draft.html.readingWidth = next; }
    prepare();
});
for (const key of Object.keys(checkboxes) as (keyof typeof checkboxes)[]) checkboxes[key].addEventListener('change', () => { draft.html[key] = checkboxes[key].checked; prepare(); });
paper.addEventListener('change', () => { draft.pdf.paper = paper.value as 'A4' | 'Letter'; prepare(); });
orientation.addEventListener('change', () => { draft.pdf.orientation = orientation.value as 'portrait' | 'landscape'; prepare(); });
retry.addEventListener('click', async () => {
    if (!value) {
        const activeSession = session;
        try {
            const captured = await capture(requestedId);
            if (activeSession !== session || !dialog.open) return;
            value = captured; setTitle(captured.name);
        } catch (error) { if (activeSession === session && dialog.open) failure(error); return; }
    }
    outputs[format]?.prepared.dispose(); delete outputs[format];
    // Clear rejected render work; never retain a failed preparation as a cache hit.
    rendered = null; prepare();
});

htmlDownload.addEventListener('click', () => {
    const output = currentOutput();
    if (!value || !output || htmlDownload.disabled) return;
    const name = `${fileSafe(value.name)}.html`;
    download(name, output.html, 'text/html;charset=utf-8'); commitSettings(draft); viewer.toast(`Downloaded ${name}`);
});
copy.addEventListener('click', async () => {
    const output = currentOutput(), activeSession = session, settings = structuredClone(draft);
    if (!output || copy.disabled) return;
    actionBusy = true; actions(true);
    try { await copyRich(output.html); if (activeSession === session && dialog.open) { commitSettings(settings); viewer.toast('Copied to clipboard'); } }
    catch { if (activeSession === session && dialog.open) viewer.toast('Failed to copy'); }
    finally { if (activeSession === session) { actionBusy = false; actions(true); } }
});
print.addEventListener('click', () => {
    if (!print.disabled) { currentOutput()?.prepared.print(); commitSettings(draft); viewer.toast('Print requested. Choose Save as PDF in the browser dialog.'); }
});
markdown.addEventListener('click', () => {
    if (!value || markdown.disabled) return;
    const name = `${fileSafe(value.name)}.md`;
    download(name, value.text, 'text/markdown;charset=utf-8'); commitSettings(draft); viewer.toast(`Downloaded ${name}`);
});
for (const id of ['exportClose', 'exportCloseFooter']) $(id).addEventListener('click', () => dialog.close());
dialog.addEventListener('close', () => {
    ++generation; ++session; renderAbort?.abort(); renderAbort = null; rendered = null;
    disposeOutputs(); value = null; actionBusy = false; dialog.returnValue = '';
});

async function capture(id: string | null) {
    if (!id || id === app.state.currentId) return viewer.captureExportSnapshot();
    const node = app.node(id);
    if (!node || node.kind !== 'doc') throw new Error('The document is no longer available.');
    const text = await db.getText(id);
    if (text === undefined) throw new Error('The document is no longer available.');
    return snapshot(id, node.name, text, viewer.exportReadingWidth());
}

export async function openExport(id: string | null = app.state.currentId, initialFormat: ExportFormat = 'html') {
    if (dialog.open) { switchFormat(initialFormat); return; }
    closeMenu();
    if (document.fullscreenElement) await document.exitFullscreen();
    const activeSession = ++session;
    // A stable opener, not the custom menu item that is removed on the next tree refresh.
    const opener = $<HTMLButtonElement>('exportBtn');
    (opener.disabled ? $('navToggle') : opener).focus();
    requestedId = id; value = null; format = initialFormat; draft = loadSettings(viewer.exportReadingWidth());
    syncFields(); syncFormat(); actions(); retry.hidden = true; $('exportChanged').hidden = true;
    setTitle(app.node(id)?.name ?? viewer.currentName());
    setStatus('Preparing preview…'); dialog.showModal();
    (format === 'html' ? htmlTab : pdfTab).focus();
    try {
        const captured = await capture(id);
        if (activeSession !== session || !dialog.open) return;
        if (!captured.body.trim()) { dialog.close(); viewer.toast('This document is empty.'); return; }
        value = captured; setTitle(captured.name);
        prepare();
    } catch (error) { if (activeSession === session && dialog.open) failure(error); }
}
$('exportBtn').addEventListener('click', () => openExport());
document.addEventListener('keydown', (event) => {
    if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'p') return;
    if (dialog.open) { event.preventDefault(); switchFormat('pdf'); return; }
    if (document.querySelector('dialog:modal') || !viewer.currentText().trim()) return;
    event.preventDefault(); openExport(app.state.currentId, 'pdf');
});

// A reload must not silently replace the captured document or switch to the current one.
app.onChange(() => {
    if (!dialog.open || !value?.id) return;
    const node = app.node(value.id);
    if (!node || node.name !== value.name || (value.id === app.state.currentId && viewer.currentText() !== value.text)) $('exportChanged').hidden = false;
    else {
        const captured = value;
        db.getText(value.id).then((text) => { if (value === captured && dialog.open && text !== captured.text) $('exportChanged').hidden = false; });
    }
});
$('exportRefresh').addEventListener('click', async () => {
    if (!value) return;
    const id = value.id, activeSession = session, refreshGeneration = ++generation;
    actions(); renderAbort?.abort(); rendered = null; disposeOutputs(); setStatus('Preparing preview…');
    try {
        const captured = await capture(id);
        if (activeSession !== session || refreshGeneration !== generation || !dialog.open) return;
        value = captured; setTitle(captured.name); $('exportChanged').hidden = true; prepare();
    } catch (error) { if (activeSession === session && dialog.open) failure(error); }
});
