export interface SourceChange { from: number; to: number; insert: string }
export function sourceChange(before: string, after: string): SourceChange | null {
    if (before === after) return null;
    let from = 0, oldEnd = before.length, newEnd = after.length;
    while (from < oldEnd && from < newEnd && before[from] === after[from]) from++;
    while (oldEnd > from && newEnd > from && before[oldEnd - 1] === after[newEnd - 1]) { oldEnd--; newEnd--; }
    if (from && /[\uD800-\uDBFF]/.test(before[from - 1])) from--;
    if (oldEnd < before.length && /[\uDC00-\uDFFF]/.test(before[oldEnd])) oldEnd++;
    if (newEnd < after.length && /[\uDC00-\uDFFF]/.test(after[newEnd])) newEnd++;
    return { from, to: oldEnd, insert: after.slice(from, newEnd) };
}
