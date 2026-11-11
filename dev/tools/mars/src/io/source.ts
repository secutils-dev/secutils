// Source extraction and naming are independent of the viewer's document/review state.
import { marked, type Tokens } from 'marked';
export const FRONTMATTER_RE = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/;
const SOURCE_EXT = /\.(?:md|markdown|mdx|txt|html?)$/i;
export const stripSourceExtension = (name: string) => name.trim().replace(SOURCE_EXT, '').trim();
export const isMarkdownFile = (name: string) => /\.(?:md|markdown|mdx|txt)$/i.test(name);

const sourceElementText = (text: string) =>
    new DOMParser().parseFromString(text, 'text/html').querySelector('script[type="text/markdown" i]')?.textContent ??
    null;
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

export function readSource(text: string, options: { name?: string; fileName?: string; fallback?: string } = {}) {
    // A fenced example of the source tag in an ordinary Markdown file is not an export.
    const html = /\.html?$/i.test(options.fileName ?? '') || /^\s*(?:<!doctype\s+html\b|<html\b)/i.test(text);
    const embedded = html ? extractEmbeddedMarkdown(text) : null;
    const source = embedded ?? text;
    return { text: source, name: sourceName(source, options), embedded: embedded !== null };
}

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

export function urlFileName(url: URL) {
    const name = url.pathname.split('/').pop() || 'document';
    try {
        return decodeURIComponent(name);
    } catch {
        return name;
    }
}
