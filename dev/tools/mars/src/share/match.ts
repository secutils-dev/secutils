// Recognizes an incoming document as an updated copy of a stored one when no origin matches
// (a moved file, or a file opened with Open). Pure; runs in Node tests.
import { splitComments } from './merge.ts';

// The distinct lines of a text, with collapsed whitespace. Lines of one or two characters are left out.
const lines = (text: string) =>
    new Set(
        text
            .split('\n')
            .map((l) => l.trim().replace(/\s+/g, ' '))
            .filter((l) => l.length > 2),
    );
// The first level-1 ATX heading, in lower case, or ''.
const title = (body: string) =>
    body
        .match(/^#\s+(.+)$/m)?.[1]
        .trim()
        .toLowerCase() ?? '';
// The document name in lower case, without a Markdown or text extension.
const stem = (name: string) =>
    name
        .trim()
        .toLowerCase()
        .replace(/\.(md|markdown|mdx|txt)$/, '');

/**
 * Share of distinct non-trivial lines the two texts have in common, relative to the longer one.
 * The result is between 0 and 1. An empty side gives 0.
 */
export function textOverlap(a: string, b: string) {
    const A = lines(a),
        B = lines(b);
    if (!A.size || !B.size) return 0;
    let common = 0;
    for (const l of A) if (B.has(l)) common++;
    return common / Math.max(A.size, B.size);
}

/**
 * 0 when unrelated; higher is likelier. Shared thread ids are decisive, since they only
 * travel with the document. Otherwise a shared name or title needs some common text, and
 * an unrelated name needs most of it.
 *
 * Thresholds: with the same name or level-1 heading, an overlap of 0.3 or more scores 0.62 to 0.9.
 * Without one, an overlap of 0.6 or more scores 0.48 to 0.8. A shared thread id scores 1.
 * The score only ranks candidates. The caller asks the reader before it merges.
 */
export function matchScore(incoming: { name: string; text: string }, stored: { name: string; text: string }) {
    const a = splitComments(incoming.text),
        b = splitComments(stored.text);
    const ids = new Set(a.threads.map((t) => t.id));
    if (b.threads.some((t) => ids.has(t.id))) return 1;
    const overlap = textOverlap(a.body, b.body);
    const named = stem(incoming.name) === stem(stored.name) || (!!title(a.body) && title(a.body) === title(b.body));
    if (named) return overlap >= 0.3 ? 0.5 + overlap / 2.5 : 0;
    return overlap >= 0.6 ? overlap * 0.8 : 0;
}
