// Changes between a reviewed baseline and the current document (pure; runs in Node tests).
// Blocks are compared first; word ranges are computed only inside edited blocks.

import { diffArrays } from 'diff';

// The review UI runs synchronously. Fall back to one changed span if the exact diff is too costly.
const DIFF_OPTIONS = { timeout: 24, maxEditLength: 1024 };
/**
 * Token limit for each side of a word comparison.
 */
const MAX_WORDS = 2000;

/**
 * Unchanged token pairs in order. Keep the common prefix and suffix even if the middle exceeds
 * the diff budget.
 */
function commonPairs(a: string[], b: string[]): [number, number][] {
    let start = 0;
    while (start < a.length && start < b.length && a[start] === b[start]) start++;
    let endA = a.length,
        endB = b.length;
    while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
        endA--;
        endB--;
    }
    const pairs: [number, number][] = [];
    for (let i = 0; i < start; i++) pairs.push([i, i]);
    if (endA > start && endB > start) {
        let i = start,
            j = start;
        for (const { added, removed, count } of diffArrays(a.slice(start, endA), b.slice(start, endB), DIFF_OPTIONS) ??
            []) {
            if (added) j += count;
            else if (removed) i += count;
            else for (let k = 0; k < count; k++) pairs.push([i++, j++]);
        }
    }
    for (let k = 0; k < a.length - endA; k++) pairs.push([endA + k, endB + k]);
    return pairs;
}

/** Exact lines, including their original terminators. Joining them recovers the source byte-for-byte. */
export const sourceLines = (value: string) => value.match(/[^\r\n]*(?:\r\n|\r|\n)|[^\r\n]+$/g) ?? [];

/** A replacement in base coordinates. An insertion has an empty base range. */
export interface TextEdit {
    start: number;
    end: number;
    replacement: string[];
    next: number;
}

/** Convert two exact token streams into bounded base-coordinate replacements. */
export function textEdits(base: string[], changed: string[]): TextEdit[] {
    const result: TextEdit[] = [];
    let start = 0,
        next = 0;
    for (const [a, b] of [...commonPairs(base, changed), [base.length, changed.length]]) {
        if (a > start || b > next) result.push({ start, end: a, replacement: changed.slice(next, b), next });
        start = a + 1;
        next = b + 1;
    }
    return result;
}

/**
 * One block-level difference. `old` indexes the baseline blocks and `cur` the current blocks.
 * A removed block's `cur` is the block it sat before (equal to the block count when it was last).
 */
export type BlockChange =
    | { kind: 'edited'; old: number; cur: number }
    | { kind: 'added'; cur: number }
    | { kind: 'removed'; old: number; cur: number };

/**
 * Compares two block lists and returns the changes in document order. Blocks match on their
 * trimmed text. Between two matching blocks, old and new blocks pair up in order as `edited`,
 * and the surplus on one side becomes `added` or `removed`.
 */
export function diffBlocks(oldRaws: string[], newRaws: string[]): BlockChange[] {
    const a = oldRaws.map((s) => s.trim()),
        b = newRaws.map((s) => s.trim());
    const out: BlockChange[] = [];
    let oi = 0,
        ni = 0;
    for (const [oj, nj] of [...commonPairs(a, b), [a.length, b.length] as [number, number]]) {
        const edited = Math.min(oj - oi, nj - ni);
        for (let k = 0; k < edited; k++) out.push({ kind: 'edited', old: oi + k, cur: ni + k });
        for (let k = ni + edited; k < nj; k++) out.push({ kind: 'added', cur: k });
        for (let k = oi + edited; k < oj; k++) out.push({ kind: 'removed', old: k, cur: nj });
        oi = oj + 1;
        ni = nj + 1;
    }
    return out;
}

/** Splits text into whitespace runs, words, and single punctuation characters. The tokens rebuild the text. */
const tokens = (text: string) => text.match(/\s+|[\p{L}\p{N}_]+|[^\s\p{L}\p{N}_]/gu) ?? [];

/**
 * Inserted `[start, end)` ranges of `newText` in UTF-16 units, or null when either side has more
 * than `MAX_WORDS` tokens. Ranges separated only by whitespace merge, and whitespace-only
 * insertions are dropped.
 */
export function diffWords(oldText: string, newText: string): [number, number][] | null {
    const a = tokens(oldText),
        b = tokens(newText);
    if (a.length > MAX_WORDS || b.length > MAX_WORDS) return null;
    const kept = new Set(commonPairs(a, b).map(([, j]) => j));
    const ranges: [number, number][] = [];
    let pos = 0;
    b.forEach((tok, j) => {
        const end = pos + tok.length;
        if (!kept.has(j) && tok.trim()) {
            const last = ranges.at(-1);
            if (last && !newText.slice(last[1], pos).trim()) last[1] = end;
            else ranges.push([pos, end]);
        }
        pos = end;
    });
    return ranges;
}
