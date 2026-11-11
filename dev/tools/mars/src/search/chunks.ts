// Pure helpers for document search: chunking for the smart-search index, plain-text
// snippets, and when to suggest smart search. Runs in Node tests.
//
// No DOM, storage, or network access. The app and the evaluation in `eval/` both run this code,
// so a change to the chunking changes the measured search quality.
// Offsets are UTF-16 indexes into the searchable body (`searchBody`). Results are plain text:
// the caller escapes them before they enter HTML.
import { splitComments } from '../share/merge.ts';

/** Leading YAML frontmatter, which is not part of the searchable body. */
const FRONTMATTER_RE = /^---\r?\n[\s\S]*?\r?\n---\r?\n/;
// Data fences (models, maps) are large and carry no prose worth embedding.
const DATA_FENCE_RE = /^(stl|su-mesh|geojson|topojson|json)\b/i;
/** Longest fenced block that gets a chunk, in characters with its fence lines. A longer block is skipped. */
const MAX_FENCE = 4000;

/**
 * Markdown reduced to readable text: markers, link targets, and HTML tags dropped.
 * Whitespace runs become one space. The patterns are heuristics, not a Markdown parser.
 */
export const plain = (md: string) =>
    md
        .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(/<[^>]+>/g, ' ')
        .replace(/^\s{0,3}(?:#{1,6}|>|[-*+]|\d+[.)])\s+/gm, '')
        .replace(/(\*\*|__|~~|==|`)/g, '')
        .replace(/\s+/g, ' ')
        .trim();

/** The searchable body: front matter and the trailing review-threads block removed, with LF line endings. */
export const searchBody = (text: string) =>
    text
        .replace(/\r\n?/g, '\n')
        .replace(FRONTMATTER_RE, '')
        .replace(/\n?<!-- su-comments\n[\s\S]*?\n-->\s*$/, '');

/**
 * One passage of a document. `start` is its offset in the searchable body, and `text` is the
 * plain text that a result shows.
 * `embed`: the text to embed when it differs from the text shown (a thread's comments
 * without the "Comment on" label, which only dilutes the match).
 */
export interface Chunk {
    start: number;
    text: string;
    embed?: string;
}

// Embeddings of one- and two-word passages ("Dog") score high against any short query,
// so such passages get no vector; text search still finds them.
const MIN_WORDS = 3;
const words = (text: string) => text.split(/\s+/).filter(Boolean).length;
/** True when a chunk has enough words (`MIN_WORDS`) for a useful vector. */
export const embeddable = (c: Chunk) => words(c.text) >= MIN_WORDS;

/**
 * One chunk per review thread, shown as `Comment on “quote”: comments` and embedded as the
 * comments alone. `start` is where the quoted text sits in `body`, or 0 when its formatting
 * hides it. With `minWords`, threads whose comments are shorter are left out.
 *
 * The quote is rendered text and `body` is Markdown source. When the whole quote has no match,
 * its first 24 characters locate the passage.
 *
 * @param text The complete document text, with its comments block.
 * @param body `searchBody(text)`.
 */
export function threadChunks(text: string, body: string, minWords = 0): Chunk[] {
    const out: Chunk[] = [];
    for (const t of splitComments(text.replace(/\r\n?/g, '\n')).threads) {
        const said = plain(t.comments.map((c) => c.body).join(' '));
        if (words(said) < minWords) continue;
        const at = body.indexOf(t.quote);
        out.push({
            start: at < 0 ? Math.max(0, body.indexOf(t.quote.slice(0, 24))) : at,
            text: `Comment on “${t.quote}”: ${said}`,
            embed: said,
        });
    }
    return out;
}

/**
 * Paragraph-sized chunks of `body` (merged up to `max` characters), each prefixed with its
 * nearest heading for context. `start` is the chunk's offset in `body`. A chunk gets one
 * vector, so a short aside inside a long chunk is lost: at 1,200 characters, "I like walking
 * with my cat" in a paragraph about network status matched neither "pet" nor "cat"
 * (eval/ measures the trade-off).
 *
 * A heading starts a chunk. A paragraph or code block longer than `max` stays whole in a chunk
 * of its own. Data fences, fences above `MAX_FENCE`, and a fence without an end get no chunk.
 */
export function chunkDoc(body: string, max = 500): Chunk[] {
    const out: Chunk[] = [];
    let heading = '',
        cur: Chunk | null = null,
        pos = 0,
        fence: { marker: string; skip: boolean; start: number } | null = null;
    // `flush` closes the open chunk. `add` appends a paragraph and closes the chunk first when the
    // paragraph does not fit.
    const flush = () => {
        if (cur?.text.trim()) out.push({ start: cur.start, text: cur.text.trim() });
        cur = null;
    };
    const add = (start: number, text: string) => {
        const t = plain(text);
        if (!t) return;
        if (cur && cur.text.length + t.length > max) flush();
        cur ??= { start, text: heading && heading !== t ? `${heading}: ` : '' };
        cur.text += `${t} `;
    };
    // First pass: split the body into paragraphs, heading lines, and whole fenced blocks.
    const paragraphs: { start: number; text: string }[] = [];
    let para: { start: number; text: string } | null = null;
    for (const line of body.split('\n')) {
        const lineStart = pos;
        pos += line.length + 1;
        const open = line.match(/^ {0,3}(`{3,}|~{3,})\s*([^\s`]*)/);
        if (fence) {
            if (open && line.trim().startsWith(fence.marker)) {
                const skip = fence.skip || pos - fence.start > MAX_FENCE;
                if (!skip) paragraphs.push({ start: fence.start, text: body.slice(fence.start, pos) });
                fence = null;
            }
            continue;
        }
        if (open) {
            if (para) {
                paragraphs.push(para);
                para = null;
            }
            fence = { marker: open[1], skip: DATA_FENCE_RE.test(open[2]), start: lineStart };
            continue;
        }
        if (!line.trim() || /^#{1,6}\s/.test(line)) {
            if (para) {
                paragraphs.push(para);
                para = null;
            }
            if (line.trim()) paragraphs.push({ start: lineStart, text: line });
            continue;
        }
        if (para) para.text += `\n${line}`;
        else para = { start: lineStart, text: line };
    }
    if (para) paragraphs.push(para);
    // Second pass: merge the paragraphs into chunks under their heading.
    for (const p of paragraphs) {
        const h = p.text.match(/^#{1,6}\s+(.+)$/);
        if (h) {
            flush();
            heading = plain(h[1]);
            add(p.start, p.text);
            continue;
        }
        add(p.start, p.text.replace(/^ {0,3}(`{3,}|~{3,}).*$/gm, ''));
    }
    flush();
    return out;
}

/**
 * A short plain-text excerpt around the first match of `query`, taken from the line that
 * contains it, or null. ponytail: a match that spans lines is not found.
 * The match ignores case. `radius` is the context in characters on each side, extended to the
 * next word boundary. An ellipsis marks a cut end.
 */
export function snippet(text: string, query: string, radius = 48) {
    const q = query.toLowerCase();
    const line = text.split('\n').find((l) => l.toLowerCase().includes(q) && plain(l).toLowerCase().includes(q));
    if (line === undefined) return null;
    const flat = plain(line),
        i = flat.toLowerCase().indexOf(q);
    const from = Math.max(0, flat.lastIndexOf(' ', Math.max(0, i - radius)) + 1);
    const to = flat.indexOf(' ', Math.min(flat.length, i + query.length + radius));
    return `${from > 0 ? '…' : ''}${flat.slice(from, to < 0 ? undefined : to)}${to < 0 ? '' : '…'}`;
}

/** The highest suggestion threshold that `count` documents reached, or 0 below the first one. */
const threshold = (count: number) =>
    count < 6 ? 0 : count < 20 ? 6 : count < 50 ? 20 : 50 + Math.floor((count - 50) / 10) * 10;
/**
 * The suggestion appears at 6, 20, and 50 documents, then at every 10 more. `shown` is the
 * threshold it last appeared at; the return value is the threshold to show now, or null.
 */
export function nudgeDue(count: number, shown: number) {
    const now = threshold(count);
    return now > shown ? now : null;
}
