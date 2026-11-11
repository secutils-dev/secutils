// Shared parts of the standalone HTML document and the print document: the head, the fonts, the
// output CSS, and the watermark. Also the helpers that embed a script or JSON in markup. The reader
// runtime bundles this module, so it imports no app, storage, or UI module. Each helper states what
// it escapes.
import outputCss from './output.css';

// Keep the standalone graph free of UI modules (and their glyph catalog).
/** Escapes text for HTML content and for a double-quoted attribute value. */
const esc = (value: string) =>
    value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

// No live app CSS, DOM, or storage is included in the standalone document.
export const FONT_LINKS =
    '<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=Inter:wght@300..700&family=Roboto+Mono:wght@400..700&display=swap" rel="stylesheet">';
/** The KaTeX stylesheet for rendered math, at the KaTeX version that the viewer renders with. */
export const KATEX_LINK =
    '<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">';
/** The stylesheet of both output documents, inlined in the head. */
export const OUTPUT_CSS = outputCss;
/** The fixed footer of both output documents. */
export const watermark =
    '<footer class="su-watermark"><a href="https://secutils.dev" target="_blank" rel="noopener">Powered by <strong>Secutils.dev</strong></a></footer>';
/**
 * Wraps trusted script source in an inline `<script>`. Each `</script` in the source gets a
 * backslash, so text inside the source cannot end the element.
 */
export const scriptTag = (source: string) => `<script>${source.replace(/<\/script/gi, '<\\/script')}</script>`;
/**
 * Embeds a value as an inert JSON data block. Each `<` becomes `\u003c`, so a string in the value
 * cannot end the element or start markup. `id` must be a fixed identifier.
 */
export const jsonTag = (id: string, value: unknown) =>
    `<script type="application/json" id="${id}">${JSON.stringify(value).replace(/</g, '\\u003c')}</script>`;
/**
 * Builds the content of `<head>`. `title` is the document name and is escaped. `content` is only
 * searched for KaTeX output, to decide on the KaTeX stylesheet. `css` is trusted extra CSS.
 */
export function head(title: string, content: string, css = '') {
    return `<meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><meta name="referrer" content="no-referrer"><title>${esc(title)}</title>${FONT_LINKS}${content.includes('class="katex') ? KATEX_LINK : ''}<style>${OUTPUT_CSS}${css}</style>`;
}
