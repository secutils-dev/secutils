// The HTML policy for every preview and export. A document, its comments, and its review
// baseline can come from another person (a link, a URL, Sync, or a Live link), and the page holds
// the library and its connection keys. No document HTML reaches `innerHTML` without this policy.
//
// DOMPurify supplies the same policy in every supported engine. The native Sanitizer API is not
// available in Safari.
import DOMPurify from 'dompurify';

/** Elements that a document must not provide: script hosts, style hosts, embeds, and form controls. */
const FORBIDDEN_TAGS = [
    'script',
    'style',
    'iframe',
    'object',
    'embed',
    'base',
    'meta',
    'link',
    'template',
    'form',
    'input',
    'button',
    'textarea',
    'select',
    'option',
    'svg',
    'math',
];
/** HTML elements without an end tag. They never stay open across Markdown tokens. */
const VOID_TAGS = new Set([
    'area',
    'base',
    'br',
    'col',
    'embed',
    'hr',
    'img',
    'input',
    'link',
    'meta',
    'param',
    'source',
    'track',
    'wbr',
]);
const TAG_RE = /<(\/?)([a-zA-Z][a-zA-Z0-9-]*)\b(?:"[^"]*"|'[^']*'|[^'">])*>/g;
const COMMENT_RE = /<!--[\s\S]*?-->/g;

const sanitizeRaw = (html: string): string =>
    DOMPurify.sanitize(html, {
        USE_PROFILES: { html: true },
        FORBID_TAGS: FORBIDDEN_TAGS,
        FORBID_ATTR: ['style', 'id', 'name', 'class', 'srcset'],
        ALLOW_DATA_ATTR: false,
        ALLOW_ARIA_ATTR: false,
        SANITIZE_NAMED_PROPS: true,
    });

const allowedTags = new Map<string, boolean>();
const allowedTag = (name: string) => {
    if (!allowedTags.has(name)) allowedTags.set(name, sanitizeRaw(`<${name}></${name}>`) !== '');
    return allowedTags.get(name)!;
};

/**
 * Finds the tags that a raw HTML fragment leaves unbalanced: `open` lists start tags without an
 * end tag (outermost first), and `orphans` lists end tags without a start tag.
 *
 * The scan only counts tags. Its result selects end tags to drop from, or add to, sanitized
 * output, so a scan error can unbalance the markup but cannot add an attribute or an element.
 */
export function unbalancedTags(html: string): { open: string[]; orphans: string[] } {
    const open: string[] = [],
        orphans: string[] = [];
    for (const [, close, rawName] of html.replace(COMMENT_RE, '').matchAll(TAG_RE)) {
        const name = rawName.toLowerCase();
        if (VOID_TAGS.has(name)) continue;
        if (!close) {
            open.push(name);
            continue;
        }
        const index = open.lastIndexOf(name);
        // An end tag also closes the elements that are still open inside its element.
        if (index < 0) orphans.push(name);
        else open.length = index;
    }
    return { open, orphans };
}

/**
 * Restores the tag balance of the source fragment in its sanitized form. The sanitizer closes
 * every element, so `<sub>` becomes `<sub></sub>` and a lone `</sub>` disappears. This function
 * drops the end tags that the sanitizer added for `open` and puts back the `orphans` end tags.
 */
export function rebalance(sanitized: string, open: string[], orphans: string[]): string {
    let html = sanitized;
    // The sanitizer serializes the outermost end tag last.
    for (const name of open) {
        const end = `</${name}>`;
        if (!html.endsWith(end)) break;
        html = html.slice(0, -end.length);
    }
    return orphans.map((name) => `</${name}>`).join('') + html;
}

/**
 * Sanitizes one raw HTML token of a Markdown document. The result has no scripts, form controls,
 * CSS, ids, classes, or `data-*` attributes, so a document cannot forge the attributes that the
 * renderer adds later (source ranges and block markers).
 *
 * @param balanced Keep the tag balance of the token. Markdown splits `H<sub>2</sub>O` into three
 *   tokens, so an inline start tag must stay open until its end tag arrives in a later token.
 */
export function safeRawHtml(html: string, balanced = false): string {
    const sanitized = sanitizeRaw(html);
    if (!balanced) return sanitized;
    const { open, orphans } = unbalancedTags(html);
    return open.length || orphans.length ? rebalance(sanitized, open, orphans.filter(allowedTag)) : sanitized;
}

/**
 * Sanitizes complete renderer output before it reaches `innerHTML`. Source-range attributes and
 * classes are allowed here because the renderer adds them after `safeRawHtml` removed every
 * document-supplied `data-*` attribute and class.
 */
export function safeRenderedHtml(html: string): string {
    return DOMPurify.sanitize(html, {
        USE_PROFILES: { html: true },
        FORBID_TAGS: FORBIDDEN_TAGS.filter((tag) => tag !== 'input'),
        FORBID_ATTR: ['style', 'name', 'srcset'],
        ALLOW_DATA_ATTR: true,
        SANITIZE_NAMED_PROPS: true,
    });
}
