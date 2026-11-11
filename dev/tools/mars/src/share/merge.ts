// Review threads in the trailing `<!-- su-comments [...] -->` block (format shared with
// Markdown Preview, see dev/tools/md/SKILL.md -> "Review comments"), and merging a shared copy back.
//
// Pure: no storage and no DOM. The block is document content, so another person can write it.
// `splitComments` checks only that the block is a JSON array. It does not check the entries.
// No function here changes its arguments.

/** One comment of a thread. `at` and `edited` are ISO 8601 times, compared as strings. */
export interface Comment {
    id?: string;
    author?: string;
    at?: string;
    body: string;
    edited?: string;
}
/**
 * One review thread. `quote` is the rendered text that the thread marks. `prefix` and `suffix`
 * are the text around it, which finds the right occurrence again.
 */
export interface Thread {
    id: string;
    quote: string;
    prefix?: string;
    suffix?: string;
    resolved?: boolean;
    comments: Comment[];
}

// The block counts only at the end of the text. Only whitespace can follow it.
const COMMENTS_RE = /(?:^|\r?\n)<!-- su-comments\r?\n([\s\S]*?)\r?\n-->\s*$/;
/**
 * Finds the trailing review block. Returns the match with `index` relative to `text` and the
 * JSON in group 1, or null. It starts at the last marker: an earlier marker, in a code sample
 * for example, cannot be the trailing block.
 */
export function matchCommentBlock(text: string) {
    const markers = /(?:^|\r?\n)<!-- su-comments\r?\n/g;
    let marker,
        start = -1;
    while ((marker = markers.exec(text))) start = marker.index;
    if (start < 0) return null;
    const match = text.slice(start).match(COMMENTS_RE);
    if (match) match.index = start + (match.index ?? 0);
    return match;
}

/**
 * Splits a document into its body and its threads. A text without a valid block is all body.
 * A block that is not a JSON array stays in the body and gives no threads.
 */
export function splitComments(text: string): { body: string; threads: Thread[] } {
    const m = matchCommentBlock(text);
    if (!m) return { body: text, threads: [] };
    try {
        const parsed = JSON.parse(m[1]);
        if (Array.isArray(parsed)) return { body: text.slice(0, m.index), threads: parsed };
    } catch {}
    return { body: text, threads: [] };
}

/**
 * Appends the review block to `body`, after one blank line. Without threads it returns `body`
 * unchanged, so a document without comments has no block.
 */
export function joinComments(body: string, threads: Thread[]) {
    if (!threads.length) return body;
    // `<` and `>` only occur inside JSON strings, so escaping them keeps `-->` out of the comment.
    const json = JSON.stringify(threads, null, 2).replace(/[<>]/g, (ch) => (ch === '<' ? '\\u003c' : '\\u003e'));
    return `${body}${body.endsWith('\n') ? '' : '\n'}\n<!-- su-comments\n${json}\n-->\n`;
}

/** The number of unresolved threads in a document text. */
export const openThreads = (text: string) => splitComments(text).threads.filter((t) => !t.resolved).length;

/** The identity of a comment in a merge. Comments written before MARS had ids are matched by author and timestamp. */
export const commentKey = (c: Comment) => c.id ?? `${c.author ?? ''}\u0000${c.at ?? ''}`;

/**
 * Union of both sides' threads and comments. A comment edited on both sides keeps the
 * later edit. ponytail: `resolved` keeps the local value for existing threads, so a
 * reopen or resolve on the other side doesn't travel back; add per-field timestamps if
 * that becomes confusing.
 * Threads match by `id`, comments by `commentKey`. `added` counts the incoming comments that
 * the local side did not have. A merged thread sorts its comments by `at`.
 */
export function mergeThreads(local: Thread[], incoming: Thread[]) {
    const out: Thread[] = structuredClone(local);
    let added = 0;
    for (const theirs of incoming) {
        const mine = out.find((t) => t.id === theirs.id);
        if (!mine) {
            out.push(structuredClone(theirs));
            added += theirs.comments.length;
            continue;
        }
        for (const c of theirs.comments) {
            const i = mine.comments.findIndex((x) => commentKey(x) === commentKey(c));
            if (i < 0) {
                mine.comments.push(structuredClone(c));
                added++;
            } else if ((c.edited ?? '') > (mine.comments[i].edited ?? '')) mine.comments[i] = structuredClone(c);
        }
        mine.comments.sort((a, b) => (a.at ?? '').localeCompare(b.at ?? ''));
    }
    return { threads: out, added };
}

/**
 * Merges a shared copy into the local document. `mine` keeps the local body, `theirs`
 * takes the incoming body; both carry the merged threads, anchored by the side whose body they keep.
 * `sameBody` compares the bodies exactly. `added` is the number of incoming comments that are
 * new. A side whose threads do not change returns its original text, byte for byte.
 */
export function mergeDocuments(localText: string, incomingText: string) {
    const local = splitComments(localText),
        incoming = splitComments(incomingText);
    const { threads, added } = mergeThreads(local.threads, incoming.threads);
    const anchors = new Map(incoming.threads.map((t) => [t.id, t]));
    const theirs = threads.map((t) => {
        const a = anchors.get(t.id);
        return a ? { ...t, quote: a.quote, prefix: a.prefix, suffix: a.suffix } : t;
    });
    // Reopening an unchanged source must not reformat its review block or line endings.
    const withThreads = (original: string, source: { body: string; threads: Thread[] }, next: Thread[]) => {
        if (JSON.stringify(source.threads) === JSON.stringify(next)) return original;
        const joined = joinComments(source.body, next);
        return original.includes('\r\n') ? joined.replace(/\r?\n/g, '\r\n') : joined;
    };
    return {
        sameBody: local.body === incoming.body,
        added,
        mine: withThreads(localText, local, threads),
        theirs: withThreads(incomingText, incoming, theirs),
    };
}
