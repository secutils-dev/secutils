// Git comparisons describe snapshots, not permanent file identities. Never guess an ambiguous move.
import type { GitHubFile } from './api.ts';
import { sourceLines } from '../doc/diff.ts';

/** A move detected by GitHub or by an unambiguous unchanged blob. */
export interface GitHubRename {
    from: string;
    to: string;
}

/** Pair deleted and added paths. Use exact blob matches first, then GitHub's similarity hints. */
export function findGitHubRenames(
    oldFiles: Pick<GitHubFile, 'path' | 'sha'>[],
    files: GitHubFile[],
    hints: GitHubRename[] = [],
) {
    const oldPaths = new Set(oldFiles.map((file) => file.path)),
        newPaths = new Set(files.map((file) => file.path));
    const gone = new Map(oldFiles.filter((file) => !newPaths.has(file.path)).map((file) => [file.path, file.sha]));
    const added = new Map(files.filter((file) => !oldPaths.has(file.path)).map((file) => [file.path, file.sha]));
    const result = new Map<string, string>(); // new -> old
    const bySha = (paths: Map<string, string>) => {
        const groups = new Map<string, string[]>();
        for (const [path, sha] of paths) {
            const group = groups.get(sha);
            if (group) group.push(path);
            else groups.set(sha, [path]);
        }
        return groups;
    };
    const oldBlobs = bySha(gone),
        newBlobs = bySha(added);
    const pair = (from: string, to: string) => {
        result.set(to, from);
        gone.delete(from);
        added.delete(to);
    };
    for (const [sha, targets] of newBlobs) {
        const sources = oldBlobs.get(sha);
        if (targets.length === 1 && sources?.length === 1) pair(sources[0], targets[0]);
    }
    const valid = hints.filter((hint) => gone.has(hint.from) && added.has(hint.to));
    const sourceCounts = new Map<string, number>(),
        targetCounts = new Map<string, number>();
    for (const hint of valid) {
        sourceCounts.set(hint.from, (sourceCounts.get(hint.from) ?? 0) + 1);
        targetCounts.set(hint.to, (targetCounts.get(hint.to) ?? 0) + 1);
    }
    for (const { from, to } of valid) {
        if (sourceCounts.get(from) === 1 && targetCounts.get(to) === 1) pair(from, to);
    }
    return result;
}

/** Bounded similarity fallback when GitHub cannot compare two heads, such as after a force-push. */
export function similarGitHubRenames(removed: Map<string, string>, added: Map<string, string>): GitHubRename[] {
    if (!removed.size || !added.size || removed.size * added.size > 1024) return [];
    const signature = (text: string) => {
        const lines = new Map<string, number>();
        for (const raw of sourceLines(text)) {
            const line = raw.trimEnd();
            if (line) lines.set(line, (lines.get(line) ?? 0) + 1);
        }
        return lines;
    };
    const old = [...removed]
        .filter(([, text]) => text.length <= 64 * 1024)
        .map(([path, text]) => [path, text, signature(text)] as const);
    const next = [...added]
        .filter(([, text]) => text.length <= 64 * 1024)
        .map(([path, text]) => [path, text, signature(text)] as const);
    const forwards = new Map<string, { path: string; score: number; tied: boolean }>();
    const backwards = new Map<string, { path: string; score: number; tied: boolean }>();
    const retain = (map: typeof forwards, from: string, to: string, score: number) => {
        const prev = map.get(from);
        if (!prev || score > prev.score) map.set(from, { path: to, score, tied: false });
        else if (score === prev.score) prev.tied = true;
    };
    for (const [from, a, left] of old)
        for (const [to, b, right] of next) {
            let shared = 0;
            for (const [line, count] of left) shared += line.length * Math.min(count, right.get(line) ?? 0);
            const score = shared / Math.max(a.length, b.length, 1);
            if (shared < 12 || score < 0.5) continue;
            retain(forwards, from, to, score);
            retain(backwards, to, from, score);
        }
    return [...forwards].flatMap(([from, match]) => {
        const back = backwards.get(match.path);
        return !match.tied && back?.path === from && !back.tied ? [{ from, to: match.path }] : [];
    });
}
