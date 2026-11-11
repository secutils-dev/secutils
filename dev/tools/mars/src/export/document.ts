import outputCss from './output.css';

// Keep the standalone graph free of UI modules (and their glyph catalog).
const esc = (value: string) =>
    value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

// No live app CSS, DOM, or storage is included in the standalone document.
export const FONT_LINKS =
    '<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=Inter:wght@300..700&family=Roboto+Mono:wght@400..700&display=swap" rel="stylesheet">';
export const KATEX_LINK =
    '<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">';
export const OUTPUT_CSS = outputCss;
export const watermark =
    '<footer class="su-watermark"><a href="https://secutils.dev" target="_blank" rel="noopener">Powered by <strong>Secutils.dev</strong></a></footer>';
export const scriptTag = (source: string) => `<script>${source.replace(/<\/script/gi, '<\\/script')}</script>`;
export const jsonTag = (id: string, value: unknown) =>
    `<script type="application/json" id="${id}">${JSON.stringify(value).replace(/</g, '\\u003c')}</script>`;
export function head(title: string, content: string, css = '') {
    return `<meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><meta name="referrer" content="no-referrer"><title>${esc(title)}</title>${FONT_LINKS}${content.includes('class="katex') ? KATEX_LINK : ''}<style>${OUTPUT_CSS}${css}</style>`;
}
