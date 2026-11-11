// Changes between a reviewed baseline and the current document (pure; runs in Node tests).
// Blocks are compared first; word ranges are computed only inside edited blocks.

// ponytail: the LCS table is O(n*m). Above this many cells a gap counts as one replacement,
// so a rewrite of a huge document marks every block in the gap as edited.
const MAX_CELLS = 4_000_000;
/**
 * Token limit for each side of a word comparison. Two sides at the limit fill exactly `MAX_CELLS`,
 * so a word comparison always uses the full table.
 */
const MAX_WORDS = 2000;

/**
 * Index pairs [i, j] with a[i] === b[j], in increasing order, forming a longest common subsequence.
 * The common prefix and suffix pair up without the table. When the remaining middle exceeds
 * `MAX_CELLS`, it contributes no pairs.
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
    const n = endA - start,
        m = endB - start;
    if (n && m && n * m <= MAX_CELLS) {
        // lcs[i][j]: LCS length of a[start+i..endA) and b[start+j..endB).
        // The cell limit keeps the shorter side at or below 2,000 entries, so a length fits in 16 bits.
        const w = m + 1,
            lcs = new Uint16Array((n + 1) * w);
        for (let i = n - 1; i >= 0; i--) {
            for (let j = m - 1; j >= 0; j--) {
                lcs[i * w + j] =
                    a[start + i] === b[start + j]
                        ? lcs[(i + 1) * w + j + 1] + 1
                        : Math.max(lcs[(i + 1) * w + j], lcs[i * w + j + 1]);
            }
        }
        for (let i = 0, j = 0; i < n && j < m;) {
            if (a[start + i] === b[start + j]) {
                pairs.push([start + i, start + j]);
                i++;
                j++;
            } else if (lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) i++;
            else j++;
        }
    }
    for (let k = 0; k < a.length - endA; k++) pairs.push([endA + k, endB + k]);
    return pairs;
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
