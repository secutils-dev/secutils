// Review threads in the trailing `<!-- su-comments [...] -->` block (format shared with
// md.html, see md.skill.md -> "Review comments"), and merging a shared copy back.
export interface Comment { id?: string; author?: string; at?: string; body: string; edited?: string }
export interface Thread { id: string; quote: string; prefix?: string; suffix?: string; resolved?: boolean; comments: Comment[] }

const COMMENTS_RE = /(?:^|\r?\n)<!-- su-comments\r?\n([\s\S]*?)\r?\n-->\s*$/;

export function splitComments(text: string): { body: string; threads: Thread[] } {
    const m = text.match(COMMENTS_RE);
    if (!m) return { body: text, threads: [] };
    try {
        const parsed = JSON.parse(m[1]);
        if (Array.isArray(parsed)) return { body: text.slice(0, m.index), threads: parsed };
    } catch {}
    return { body: text, threads: [] };
}

export function joinComments(body: string, threads: Thread[]) {
    if (!threads.length) return body;
    // `<` and `>` only occur inside JSON strings, so escaping them keeps `-->` out of the comment.
    const json = JSON.stringify(threads, null, 2).replace(/[<>]/g, (ch) => (ch === '<' ? '\\u003c' : '\\u003e'));
    return `${body}${body.endsWith('\n') ? '' : '\n'}\n<!-- su-comments\n${json}\n-->\n`;
}

export const openThreads = (text: string) => splitComments(text).threads.filter((t) => !t.resolved).length;

// Comments written before MARS had ids are matched by author and timestamp.
export const commentKey = (c: Comment) => c.id ?? `${c.author ?? ''}\u0000${c.at ?? ''}`;

// Union of both sides' threads and comments. A comment edited on both sides keeps the
// later edit. ponytail: `resolved` keeps the local value for existing threads, so a
// reopen or resolve on the other side doesn't travel back; add per-field timestamps if
// that becomes confusing.
export function mergeThreads(local: Thread[], incoming: Thread[]) {
    const out: Thread[] = structuredClone(local);
    let added = 0;
    for (const theirs of incoming) {
        const mine = out.find((t) => t.id === theirs.id);
        if (!mine) { out.push(structuredClone(theirs)); added += theirs.comments.length; continue; }
        for (const c of theirs.comments) {
            const i = mine.comments.findIndex((x) => commentKey(x) === commentKey(c));
            if (i < 0) { mine.comments.push(structuredClone(c)); added++; }
            else if ((c.edited ?? '') > (mine.comments[i].edited ?? '')) mine.comments[i] = structuredClone(c);
        }
        mine.comments.sort((a, b) => (a.at ?? '').localeCompare(b.at ?? ''));
    }
    return { threads: out, added };
}

// Merges a shared copy into the local document. `mine` keeps the local body, `theirs`
// takes the incoming body; both carry the merged threads, anchored by the side whose body they keep.
export function mergeDocuments(localText: string, incomingText: string) {
    const local = splitComments(localText), incoming = splitComments(incomingText);
    const { threads, added } = mergeThreads(local.threads, incoming.threads);
    const anchors = new Map(incoming.threads.map((t) => [t.id, t]));
    const theirs = threads.map((t) => {
        const a = anchors.get(t.id);
        return a ? { ...t, quote: a.quote, prefix: a.prefix, suffix: a.suffix } : t;
    });
    return {
        sameBody: local.body === incoming.body,
        added,
        mine: joinComments(local.body, threads),
        theirs: joinComments(incoming.body, theirs),
    };
}
