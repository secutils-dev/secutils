import readerRuntime from 'mars-runtime:reader';
import { head, jsonTag, scriptTag, watermark } from './document.ts';
import { icon } from '../ui/icons.ts';
import type { ExportSnapshot } from './snapshot.ts';
import type { ExportSettings } from './settings.ts';

export const embedSource = (body: string) =>
    `<script type="text/markdown" id="su-md-source" data-source="markdown">\n${body.replace(/<\/(script)/gi, '<\\/$1')}\n</script>`;
export function hasToc(content: string) {
    return (content.match(/<h[23]\b/gi)?.length ?? 0) > 2;
}

export function htmlDocument(value: ExportSnapshot, content: string, settings: ExportSettings, operation: string) {
    const { html, pdf } = settings;
    const width = html.readingWidth === 'full' ? 'none' : `${html.readingWidth}px`;
    return `<!DOCTYPE html><html lang="en" data-theme="light" style="--su-reading-width:${width}"><head>${head(value.name, content)}</head><body>
<div id="progress"></div><div id="top-tools">
${html.find ? `<button id="find-btn" type="button" aria-label="Find in page">${icon('magnify')}</button>` : ''}
${html.pdf ? `<button id="pdf-btn" type="button" aria-label="Prepare PDF">${icon('download')}</button>` : ''}
<button id="reading-settings-btn" type="button" commandfor="reading-settings" command="toggle-popover" aria-haspopup="dialog" aria-label="Reading settings">${icon('gear')}</button>
<button id="theme-toggle" type="button" aria-label="Toggle theme"><span class="icon-sun">${icon('sun')}</span><span class="icon-moon">${icon('moon')}</span></button></div>
<div id="reading-settings" popover role="dialog" aria-label="Reading settings"><label for="reader-width-select">Reading width</label><select id="reader-width-select"><option value="800">Comfortable (800px)</option><option value="1100">Wide (1100px)</option><option value="full">Full width</option><option value="custom">Custom</option></select><div id="reader-custom-wrap" hidden><label for="reader-custom-width">Custom width (px)</label><input id="reader-custom-width" type="number" min="320" max="2400" step="1" inputmode="numeric"></div><button id="reader-width-reset" type="button">Use document default</button></div>
${html.find ? '<search id="su-find" hidden><input type="search" placeholder="Find in page" aria-label="Find in page"><span class="su-find-count" role="status">0/0</span><button type="button" class="su-find-prev" aria-label="Previous match">↑</button><button type="button" class="su-find-next" aria-label="Next match">↓</button><button type="button" class="su-find-close" aria-label="Close find">×</button></search>' : ''}
<button id="btt" type="button" aria-label="Back to top" hidden>↑</button>
${html.toc && hasToc(content) ? '<nav id="toc" aria-label="Table of contents"></nav>' : ''}<main><article>${content}</article></main>${watermark}
<div id="reader-toast" class="reader-toast" role="status" hidden></div>
${jsonTag('mars-reader-config', { operation, readingWidth: html.readingWidth, pdf })}${scriptTag(readerRuntime)}${html.source ? embedSource(value.body) : ''}</body></html>`;
}
