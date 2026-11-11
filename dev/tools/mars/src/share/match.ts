// Recognizes an incoming document as an updated copy of a stored one when no origin matches
// (a moved file, or a file opened with Open). Pure; runs in Node tests.
import { splitComments } from './merge.ts';

const lines = (text: string) => new Set(text.split('\n').map((l) => l.trim().replace(/\s+/g, ' ')).filter((l) => l.length > 2));
const title = (body: string) => body.match(/^#\s+(.+)$/m)?.[1].trim().toLowerCase() ?? '';
const stem = (name: string) => name.trim().toLowerCase().replace(/\.(md|markdown|mdx|txt)$/, '');

// Share of distinct non-trivial lines the two texts have in common, relative to the longer one.
export function textOverlap(a: string, b: string) {
    const A = lines(a), B = lines(b);
    if (!A.size || !B.size) return 0;
    let common = 0;
    for (const l of A) if (B.has(l)) common++;
    return common / Math.max(A.size, B.size);
}

// 0 when unrelated; higher is likelier. Shared thread ids are decisive, since they only
// travel with the document. Otherwise a shared name or title needs some common text, and
// an unrelated name needs most of it.
export function matchScore(incoming: { name: string; text: string }, stored: { name: string; text: string }) {
    const a = splitComments(incoming.text), b = splitComments(stored.text);
    const ids = new Set(a.threads.map((t) => t.id));
    if (b.threads.some((t) => ids.has(t.id))) return 1;
    const overlap = textOverlap(a.body, b.body);
    const named = stem(incoming.name) === stem(stored.name) || (!!title(a.body) && title(a.body) === title(b.body));
    if (named) return overlap >= 0.3 ? 0.5 + overlap / 2.5 : 0;
    return overlap >= 0.6 ? overlap * 0.8 : 0;
}
