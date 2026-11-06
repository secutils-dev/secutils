// Bundled independently and inlined into the opaque print frame. No MARS imports.
export {};
const config = JSON.parse(document.getElementById('mars-print-config')!.textContent!);
const send = (type: string, extra = {}) => parent.postMessage({ kind: 'mars-export', operation: config.operation, type, ...extra }, '*');
let ready = false;
const print = () => {
    if (!ready) return;
    send('print-requested');
    window.print();
};
window.addEventListener('message', (event) => {
    if (event.source === parent && event.data?.kind === 'mars-export' && event.data.operation === config.operation && event.data.type === 'print') print();
});
window.addEventListener('afterprint', () => send('afterprint'));

async function paginate() {
    // Paged.js is the only export-specific external JS dependency. Never fetched by HTML
    // preview, app startup, or Markdown download. Browser HTTP caching serves subsequent frames.
    const win = window as unknown as { PagedConfig: { auto: boolean }; Paged: { Previewer: new () => { preview(): Promise<{ total: number }> } } };
    win.PagedConfig = { auto: false };
    await new Promise<void>((resolve, reject) => {
        const script = document.createElement('script');
        script.src = 'https://cdn.jsdelivr.net/npm/pagedjs@0.4.3/dist/paged.polyfill.js';
        script.crossOrigin = 'anonymous'; script.setAttribute('fetchpriority', 'low');
        script.onload = () => resolve();
        script.onerror = () => reject(new Error('Paged.js could not load. HTML and Markdown downloads are still available.'));
        document.head.append(script);
    });
    for (const image of document.images) {
        image.loading = 'eager';
        if (!image.complete) await new Promise<void>((resolve, reject) => {
            image.onload = () => resolve(); image.onerror = () => reject(new Error(`An image could not load: ${image.alt || 'document image'}`));
        });
        if (!image.naturalWidth) throw new Error(`An image could not load: ${image.alt || 'document image'}`);
    }
    // SVG basemap images do not appear in document.images. Tiles are optional: retain
    // vector map geometry if a tile fails, but wait for available tiles before layout.
    await Promise.all([...document.querySelectorAll('svg image')].map((element) => new Promise<void>((resolve) => {
        const url = element.getAttribute('href') ?? element.getAttribute('xlink:href');
        if (!url) { resolve(); return; }
        const image = new Image(); image.onload = () => resolve(); image.onerror = () => resolve(); image.src = url;
    })));
    await document.fonts.ready;
    const result = await new win.Paged.Previewer().preview();
    if (!result.total || !document.querySelector('.pagedjs_page')) throw new Error('Pagination produced no pages.');
    ready = true;
    // Append screen-only chrome after Paged.js processes the print styles. Its CSS
    // parser rewrites media rules for page preparation; these must retain native media.
    const screenStyle = document.createElement('style');
    screenStyle.textContent = `@media screen {
        body { background: #e5e7eb; overflow-x: hidden; }
        .pagedjs_pages { transform-origin: top left; transform: scale(var(--page-scale, 1)); }
        .pagedjs_page { background: #fff; margin-bottom: 16px; box-shadow: 0 2px 8px #0002; }
        #print-frame-action { position: fixed; top: 8px; right: 8px; z-index: 100; padding: 6px 12px; font: 13px system-ui; }
    }
    @media print { body { height: auto !important; } .pagedjs_pages { transform: none !important; } .pagedjs_page { margin: 0; box-shadow: none; } #print-frame-action { display: none !important; } }`;
    document.head.append(screenStyle);
    // A directly activated child control also works when a browser blocks parent printing.
    const action = document.createElement('button');
    action.type = 'button'; action.id = 'print-frame-action'; action.textContent = 'Print / Save PDF'; action.addEventListener('click', print);
    document.body.append(action);
    const pages = document.querySelector<HTMLElement>('.pagedjs_pages')!;
    const scale = () => {
        const width = config.width * 96 / 25.4;
        const factor = Math.min(1, window.innerWidth / width);
        pages.style.setProperty('--page-scale', String(factor));
        document.body.style.height = `${pages.offsetHeight * factor}px`;
    };
    scale(); window.addEventListener('resize', scale);
    send('ready', { pages: result.total });
}
paginate().catch((error) => send('error', { message: error.message }));
