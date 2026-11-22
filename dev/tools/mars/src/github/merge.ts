// Three-way comparison of exact UTF-8 source. Remote and local may have changed independently.
import { diff3Merge } from 'node-diff3';
import { sourceLines } from '../doc/diff.ts';

// Limit repeated-token matches: a document with many identical lines can be more costly than
// a larger document with distinct lines. An oversized middle becomes a manual conflict.
const MAX_MERGE_TOKENS = 20_000;
const MAX_TOKEN_MATCHES = 1_000_000;

/** An automatically merged segment, or a conflict with all three exact variants. */
export type MergePart = { text: string } | { base: string; local: string; remote: string };

/** Private persisted conflict snapshot. No marker text is inserted into the working document. */
export interface GitHubConflict {
    base: string;
    local: string;
    remote: string;
    sha: string;
}

/** Decide whether to acknowledge, pull, push, or preserve a conflict without overwriting either edit. */
export function fileDecision(base: string | undefined, local: string, remote: string) {
    if (local === remote) return 'ack';
    if (local === base) return 'pull';
    if (remote === base) return 'push';
    return 'conflict';
}

const words = (value: string) => value.match(/\r\n|[\p{L}\p{N}_]+|[^\p{L}\p{N}_]/gu) ?? [];

function withinBudget(base: string[], local: string[], remote: string[]) {
    if (base.length + local.length + remote.length > MAX_MERGE_TOKENS) return false;
    const counts = new Map<string, number>();
    for (const token of base) counts.set(token, (counts.get(token) ?? 0) + 1);
    let matches = 0;
    for (const side of [local, remote])
        for (const token of side) {
            matches += counts.get(token) ?? 0;
            if (matches > MAX_TOKEN_MATCHES) return false;
        }
    return true;
}

function mergeLines(base: string[], local: string[], remote: string[]): MergePart[] {
    const parts: MergePart[] = [];
    const append = (text: string) => {
        if (!text) return;
        const previous = parts.at(-1);
        if (previous && 'text' in previous) previous.text += text;
        else parts.push({ text });
    };
    const same = (a: string, b: string, c: string) => a !== undefined && a === b && a === c;
    let start = 0,
        end = 0;
    while (same(base[start], local[start], remote[start])) start++;
    const limit = Math.min(base.length, local.length, remote.length) - start;
    while (end < limit && same(base.at(-1 - end)!, local.at(-1 - end)!, remote.at(-1 - end)!)) end++;
    append(base.slice(0, start).join(''));
    const a = local.slice(start, local.length - end),
        o = base.slice(start, base.length - end),
        b = remote.slice(start, remote.length - end);
    const conflictPart = (mine: string[], original: string[], theirs: string[]) => ({
        base: original.join(''),
        local: mine.join(''),
        remote: theirs.join(''),
    });
    if (!withinBudget(o, a, b)) parts.push(conflictPart(a, o, b));
    else
        for (const region of diff3Merge(a, o, b)) {
            if (region.ok) {
                append(region.ok.join(''));
                continue;
            }
            const { a: mine, o: original, b: theirs } = region.conflict;
            // Diff3 groups adjacent line edits. Refine only a small hunk, preserving exact
            // separators and letting Diff3 decide whether its finer-grained changes overlap.
            const hunk = conflictPart(mine, original, theirs);
            if (original.length && [hunk.base, hunk.local, hunk.remote].every((value) => value.length <= 1024)) {
                const inner = diff3Merge(words(hunk.local), words(hunk.base), words(hunk.remote));
                if (inner.every((part) => part.ok)) {
                    append(inner.map((part) => part.ok!.join('')).join(''));
                    continue;
                }
            }
            parts.push(hunk);
        }
    if (end) append(base.slice(-end).join(''));
    return parts;
}

/** Merge independent changes; preserve contested ranges for an explicit decision. */
export function mergeGitHubText(base: string, local: string, remote: string): MergePart[] {
    if (base === local || local === remote) return [{ text: remote }];
    if (base === remote) return [{ text: local }];
    return mergeLines(sourceLines(base), sourceLines(local), sourceLines(remote));
}
