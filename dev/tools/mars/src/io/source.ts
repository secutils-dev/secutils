// Source extraction and naming are independent of the viewer's document/review state.
//
// Open, drops, imports, Load URL, `?url=`, and links all read their source through this module.
// One rule therefore decides the text and the name of an incoming document.
// The input is untrusted text. This module returns strings only: it never renders the text, and
// it parses HTML with `DOMParser`, whose documents do not run scripts.
import { marked, type Tokens } from 'marked';
/** YAML frontmatter at the start of a document, after an optional byte-order mark. Group 1 is its content. */
export const FRONTMATTER_RE = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/;
// File extensions that a document name does not keep.
const SOURCE_EXT = /\.(?:md|markdown|mdx|txt|html?)$/i;
/** `name` without outer whitespace and without a source file extension. */
export const stripSourceExtension = (name: string) => name.trim().replace(SOURCE_EXT, '').trim();
/** True for a file name with a Markdown or plain-text extension. HTML files do not count. */
export const isMarkdownFile = (name: string) => /\.(?:md|markdown|mdx|txt)$/i.test(name);

// The text of the first `<script type="text/markdown">` element, or null.
const sourceElementText = (text: string) =>
    new DOMParser().parseFromString(text, 'text/html').querySelector('script[type="text/markdown" i]')?.textContent ??
    null;
/**
 * Returns the Markdown source that a MARS HTML export embeds, or null when there is none.
 * The regular-expression test skips the HTML parse for a text without such a tag.
 * `readElement` replaces the `DOMParser` lookup where no DOM exists, such as the Node tests.
 */
export function extractEmbeddedMarkdown(
    text: string,
    readElement: (text: string) => string | null = sourceElementText,
): string | null {
    if (!/<script\b[^>]*\btype\s*=\s*(?:["']text\/markdown["']|text\/markdown(?=\s|>))/i.test(text)) return null;
    const content = readElement(text);
    if (content === null) return null;
    // The standalone exporter adds one wrapping newline and escapes closing script tags.
    return content
        .replace(/<\\\/(script)/gi, '</$1')
        .replace(/^\r?\n/, '')
        .replace(/\r?\n$/, '');
}

/**
 * Returns the `title` of the frontmatter, or null. It reads a plain value, a double-quoted
 * value (JSON escapes), and a single-quoted value (`''` is one quote). A block scalar
 * (`|` or `>`) gives null.
 */
export function frontmatterTitle(text: string) {
    const raw = text
        .match(FRONTMATTER_RE)?.[1]
        .match(/^title:[ \t]*(.+?)[ \t]*\r?$/m)?.[1]
        ?.trim();
    if (!raw || raw === '|' || raw === '>') return null;
    if (raw.startsWith('"') && raw.endsWith('"')) {
        try {
            return String(JSON.parse(raw)).trim() || null;
        } catch {}
    }
    if (raw.startsWith("'") && raw.endsWith("'")) return raw.slice(1, -1).replaceAll("''", "'").trim() || null;
    return raw;
}

/**
 * Picks the document name. The first available source wins: `options.name`, the frontmatter
 * title, the first Markdown heading (at most 120 characters), `options.fileName`,
 * `options.fallback`, then "Untitled". The name and the file name lose their source extension.
 */
export function sourceName(text: string, options: { name?: string; fileName?: string; fallback?: string } = {}) {
    const explicit = options.name && stripSourceExtension(options.name);
    if (explicit) return explicit;
    const title = frontmatterTitle(text);
    if (title) return title;
    const body = text.replace(FRONTMATTER_RE, '').replace(/^\uFEFF/, '');
    // The lexer ignores headings in code fences and HTML blocks and handles setext headings.
    // Huge model fences contain no document heading and do not need another parser pass.
    const model = body.length > 1000000 && /^(`{3,}|~{3,})(?:stl|su-mesh)[^\r\n]*\r?\n[\s\S]*\r?\n\1\s*$/.test(body);
    const heading = model
        ? null
        : marked
              .lexer(body)
              .find((token): token is Tokens.Heading => token.type === 'heading')
              ?.text?.trim();
    return (
        heading?.slice(0, 120) ||
        (options.fileName && stripSourceExtension(options.fileName)) ||
        options.fallback ||
        'Untitled'
    );
}

/**
 * Returns the text and the name of an incoming source.
 * HTML is recognized by the file extension, or by a leading doctype or `<html>` tag.
 * For HTML with an embedded Markdown source, `text` is that source and `embedded` is true.
 * Other input stays as it is.
 */
export function readSource(text: string, options: { name?: string; fileName?: string; fallback?: string } = {}) {
    // A fenced example of the source tag in an ordinary Markdown file is not an export.
    const html = /\.html?$/i.test(options.fileName ?? '') || /^\s*(?:<!doctype\s+html\b|<html\b)/i.test(text);
    const embedded = html ? extractEmbeddedMarkdown(text) : null;
    const source = embedded ?? text;
    return { text: source, name: sourceName(source, options), embedded: embedded !== null };
}

/** Parses a user-supplied address. Throws an `Error` with a display message unless it is an http(s) URL. */
export function sourceUrl(value: string) {
    let url: URL;
    try {
        url = new URL(value);
    } catch {
        throw new Error('Enter a valid URL');
    }
    if (!['https:', 'http:'].includes(url.protocol)) throw new Error('Only http(s) URLs are supported');
    return url;
}

/** The last path segment of `url`, percent-decoded when possible, or "document" when the path has none. */
export function urlFileName(url: URL) {
    const name = url.pathname.split('/').pop() || 'document';
    try {
        return decodeURIComponent(name);
    } catch {
        return name;
    }
}
