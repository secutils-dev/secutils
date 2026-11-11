// Independent standalone reader. No viewer, library storage, analytics, or model importer.
import { initDiagrams } from '../doc/diagrams.js';
import { prepareFrame, printDocument, type PreparedFrame } from './pdf.ts';

// Select the script element itself: a document heading can produce the same id.
const config = JSON.parse(document.querySelector('script#mars-reader-config')!.textContent!);
const root = document.documentElement;
const article = document.querySelector('article')!;
// Capture before reader navigation/find/copy decorations can change the print input.
const originalContent = article.innerHTML;
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const send = (type: string, extra = {}) => {
    if (parent !== window) parent.postMessage({ kind: 'mars-export', operation: config.operation, type, ...extra }, '*');
};
let toastTimer: ReturnType<typeof setTimeout>;
const toast = (text: string) => {
    const el = $('reader-toast'); el.textContent = text; el.hidden = false;
    clearTimeout(toastTimer); toastTimer = setTimeout(() => { el.hidden = true; }, 2200);
};

$('theme-toggle').addEventListener('click', () => { root.dataset.theme = root.dataset.theme === 'dark' ? 'light' : 'dark'; });
if (matchMedia('(prefers-color-scheme: dark)').matches) root.dataset.theme = 'dark';

const select = $<HTMLSelectElement>('reader-width-select');
const custom = $<HTMLInputElement>('reader-custom-width');
let lastCustom = typeof config.readingWidth === 'number' ? config.readingWidth : 800;
const setWidth = (width: number | 'full') => root.style.setProperty('--su-reading-width', width === 'full' ? 'none' : `${width}px`);
const syncWidth = (width: number | 'full') => {
    select.value = width === 'full' ? 'full' : width === 800 || width === 1100 ? String(width) : 'custom';
    $('reader-custom-wrap').hidden = select.value !== 'custom';
    if (typeof width === 'number') lastCustom = width;
    custom.value = String(lastCustom);
};
syncWidth(config.readingWidth);
select.addEventListener('change', () => {
    $('reader-custom-wrap').hidden = select.value !== 'custom';
    setWidth(select.value === 'custom' ? lastCustom : select.value === 'full' ? 'full' : Number(select.value));
    if (select.value === 'custom') custom.focus();
});
custom.addEventListener('input', () => { if (custom.value && custom.validity.valid) { lastCustom = Number(custom.value); setWidth(lastCustom); } });
$('reader-width-reset').addEventListener('click', () => { setWidth(config.readingWidth); syncWidth(config.readingWidth); });
if (!('commandForElement' in HTMLButtonElement.prototype)) {
    const button = $('reading-settings-btn'), panel = $('reading-settings');
    button.addEventListener('click', () => panel.matches(':popover-open') ? panel.hidePopover() : panel.showPopover());
}

const toc = $('toc');
if (toc) {
    const title = document.createElement('div'); title.className = 'toc-title'; title.textContent = 'Contents';
    const list = document.createElement('ul');
    for (const [index, heading] of [...article.querySelectorAll('h2, h3')].entries()) {
        heading.id ||= `section-${index}`;
        const item = document.createElement('li'); item.className = heading.tagName === 'H3' ? 'sub' : '';
        const link = document.createElement('a'); link.href = `#${heading.id}`;
        link.textContent = heading.textContent!.replace(/#$/, '').trim(); item.append(link); list.append(item);
    }
    toc.append(title, list); document.body.classList.add('has-toc');
}

const find = $('su-find');
if (find) {
    const input = find.querySelector('input')!;
    const count = find.querySelector('.su-find-count')!;
    let hits: HTMLElement[] = [], current = -1;
    const clear = () => { for (const hit of hits) hit.replaceWith(document.createTextNode(hit.textContent!)); hits = []; current = -1; article.normalize(); };
    const move = (to: number) => {
        hits[current]?.classList.remove('current'); current = hits.length ? (to + hits.length) % hits.length : -1;
        hits[current]?.classList.add('current'); hits[current]?.scrollIntoView({ block: 'center' });
        count.textContent = hits.length ? `${current + 1}/${hits.length}` : '0/0';
    };
    const search = () => {
        clear(); const needle = input.value.trim().toLowerCase();
        if (!needle) { move(0); return; }
        const walker = document.createTreeWalker(article, NodeFilter.SHOW_TEXT, { acceptNode: (node) =>
            node.parentElement?.closest('script, style, svg, .katex, .su-diagram, .copy-btn') || !node.textContent!.toLowerCase().includes(needle) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT });
        const nodes: Node[] = []; while (walker.nextNode()) nodes.push(walker.currentNode);
        for (const node of nodes) {
            const text = node.textContent!, lower = text.toLowerCase(), fragment = document.createDocumentFragment();
            let from = 0, at: number;
            while ((at = lower.indexOf(needle, from)) >= 0) {
                fragment.append(document.createTextNode(text.slice(from, at)));
                const mark = document.createElement('mark'); mark.className = 'su-find-hit'; mark.textContent = text.slice(at, at + needle.length);
                hits.push(mark); fragment.append(mark); from = at + needle.length;
            }
            fragment.append(document.createTextNode(text.slice(from))); (node as ChildNode).replaceWith(fragment);
        }
        move(0);
    };
    const open = () => { find.hidden = false; input.focus(); input.select(); search(); };
    const close = () => { find.hidden = true; clear(); count.textContent = '0/0'; $('find-btn').focus(); };
    $('find-btn').addEventListener('click', () => find.hidden ? open() : close());
    find.querySelector('.su-find-close')!.addEventListener('click', close);
    find.querySelector('.su-find-prev')!.addEventListener('click', () => move(current - 1));
    find.querySelector('.su-find-next')!.addEventListener('click', () => move(current + 1));
    input.addEventListener('input', search);
    input.addEventListener('keydown', (event) => { if (event.key === 'Enter') { event.preventDefault(); move(current + (event.shiftKey ? -1 : 1)); } else if (event.key === 'Escape') close(); });
    document.addEventListener('keydown', (event) => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') { event.preventDefault(); open(); } });
}

for (const pre of article.querySelectorAll('pre')) {
    pre.tabIndex = 0;
    const button = document.createElement('button'); button.type = 'button'; button.className = 'copy-btn'; button.textContent = 'Copy';
    button.addEventListener('click', async () => {
        try { await navigator.clipboard.writeText(pre.querySelector('code')?.textContent ?? pre.textContent!.replace(/Copy$/, '')); toast('Copied to clipboard'); }
        catch { toast('Failed to copy'); }
    });
    pre.append(button);
}
const progress = $('progress'), back = $('btt');
window.addEventListener('scroll', () => {
    const max = root.scrollHeight - innerHeight;
    progress.style.width = `${max > 0 ? window.scrollY / max * 100 : 0}%`; back.hidden = window.scrollY < 300;
}, { passive: true });
back.addEventListener('click', () => window.scrollTo({ top: 0, behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth' }));

const pdfButton = $<HTMLButtonElement>('pdf-btn');
if (pdfButton) pdfButton.addEventListener('click', () => {
    const dialog = document.createElement('dialog');
    dialog.setAttribute('aria-label', 'Print / Save PDF');
    dialog.className = 'reader-print-dialog';
    const status = document.createElement('p'); status.textContent = 'Preparing PDF…'; status.setAttribute('role', 'status');
    const preview = document.createElement('div');
    const action = document.createElement('button'); action.type = 'button'; action.textContent = 'Print / Save PDF'; action.disabled = true;
    const close = document.createElement('button'); close.type = 'button'; close.textContent = 'Close'; close.addEventListener('click', () => dialog.close());
    dialog.append(status, action, close, preview); document.body.append(dialog); dialog.showModal();
    pdfButton.disabled = true;
    let prepared: PreparedFrame | null = null;
    const operation = crypto.randomUUID();
    try {
        prepared = prepareFrame(preview, printDocument(originalContent, document.title, config.pdf, operation), operation, 'pdf', undefined, () => {
            setTimeout(() => { if (dialog.open) action.focus(); }, 0);
        });
        prepared.ready.then(() => { action.disabled = false; status.textContent = 'Ready. Use the browser print dialog to save a PDF.'; }, (error) => { if (dialog.open) status.textContent = error.message; });
        action.addEventListener('click', () => { prepared?.print(); toast('Print requested. Choose Save as PDF in the browser dialog.'); });
    } catch (error) { status.textContent = (error as Error).message; }
    dialog.addEventListener('close', () => { prepared?.dispose(); dialog.remove(); pdfButton.disabled = false; }, { once: true });
});

try { initDiagrams(document); send('ready'); }
catch (error) { send('error', { message: (error as Error).message }); }
