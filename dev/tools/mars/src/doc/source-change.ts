// The smallest single replacement that turns one text into another. The source editor applies a
// remote update as this one change, so a selection outside it stays in place. The Sync replica
// records a local edit as this one change. Pure: no DOM and no storage access.

/** Replace `[from, to)` of the old text with `insert`. Offsets are UTF-16 code units. */
export interface SourceChange {
    from: number;
    to: number;
    insert: string;
}

/**
 * Returns the change from `before` to `after` after it trims their common prefix and suffix, or
 * null when the texts are equal. The change never splits a surrogate pair.
 */
export function sourceChange(before: string, after: string): SourceChange | null {
    if (before === after) return null;
    let from = 0,
        oldEnd = before.length,
        newEnd = after.length;
    while (from < oldEnd && from < newEnd && before[from] === after[from]) from++;
    while (oldEnd > from && newEnd > from && before[oldEnd - 1] === after[newEnd - 1]) {
        oldEnd--;
        newEnd--;
    }
    // A shared high or low surrogate can belong to two different characters. Widen the change to
    // whole pairs, so neither side of the splice holds half a character.
    if (from && /[\uD800-\uDBFF]/.test(before[from - 1])) from--;
    if (oldEnd < before.length && /[\uDC00-\uDFFF]/.test(before[oldEnd])) oldEnd++;
    if (newEnd < after.length && /[\uDC00-\uDFFF]/.test(after[newEnd])) newEnd++;
    return { from, to: oldEnd, insert: after.slice(from, newEnd) };
}
