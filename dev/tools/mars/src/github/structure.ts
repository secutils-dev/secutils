// Durable GitHub rename/delete intents. Paths and expected blob IDs refer to an exact Git tree.
import type { GitHubTreeEntry } from './api.ts';

/** Local tree changed before the pull transaction could apply. Retry without network backoff. */
export const LOCAL_STRUCTURE_CHANGED = 'A local GitHub change was queued during this check. Sync again to include it.';

/** One working file in an operation. Its local text is retained until deletion is acknowledged. */
export interface StructuralFile {
    id: string;
    from: string;
    sha: string;
    to?: string;
    local?: string;
    attemptedSha?: string;
}

/** Why an operation needs attention. Upstream identifiers never authorize a write on their own. */
export type StructuralBlock =
    | { reason: 'destination'; path: string }
    | { reason: 'renamed'; path: string }
    | { reason: 'source'; path: string; sha?: string }
    | { reason: 'outside' | 'limit' | 'excluded'; path: string };

/** A subtree operation; its optimistic name/visibility and the intent commit together. */
export interface StructuralJob {
    id: string;
    node: string;
    kind: 'rename' | 'delete';
    ids: string[];
    name: string;
    from: string;
    to?: string;
    folder: boolean;
    files: StructuralFile[];
    blocked?: StructuralBlock;
}

/** What a browser can safely submit against the advertised branch head. */
export type StructuralPlan =
    { blocked: StructuralBlock } | { complete: true } | { complete: false; changes: StructuralFile[] };

/** Detect a different upstream name only with a one-to-one match of the unchanged tracked blobs.
 * Both names must share a parent so resolving the name does not also relocate the working tree. */
export function upstreamRename(job: StructuralJob, tree: Map<string, GitHubTreeEntry>): string | undefined {
    if (
        job.kind !== 'rename' ||
        !job.files.length ||
        tree.has(job.from) ||
        job.files.some((file) => tree.has(file.from))
    )
        return;
    const parent = job.from.slice(0, job.from.lastIndexOf('/') + 1);
    const first = job.files[0],
        suffix = first.from.slice(job.from.length);
    const candidates = [...tree.values()]
        .filter(
            (entry) =>
                entry.type === 'blob' &&
                entry.mode === '100644' &&
                entry.sha === first.sha &&
                entry.path.endsWith(suffix),
        )
        .map((entry) => entry.path.slice(0, entry.path.length - suffix.length));
    const matches = candidates.filter(
        (path) =>
            path !== job.from &&
            path.slice(0, path.lastIndexOf('/') + 1) === parent &&
            job.files.every((file) => {
                const entry = tree.get(`${path}${file.from.slice(job.from.length)}`);
                return entry?.type === 'blob' && entry.mode === '100644' && entry.sha === file.sha;
            }),
    );
    return matches.length === 1 ? matches[0] : undefined;
}

/** Recheck a durable intent against a complete tree, including files MARS did not import. */
export function planGitHubStructure(job: StructuralJob, tree: Map<string, GitHubTreeEntry>): StructuralPlan {
    const from = new Set(job.files.map((file) => file.from));
    if (job.folder) {
        for (const entry of tree.values())
            if (
                entry.type !== 'tree' &&
                (entry.path === job.from || entry.path.startsWith(`${job.from}/`)) &&
                !from.has(entry.path)
            )
                return { blocked: { reason: 'outside', path: entry.path } };
    }
    if (
        job.kind === 'rename' &&
        job.files.length &&
        job.files.every(
            (file) =>
                !tree.has(file.from) &&
                file.attemptedSha &&
                tree.get(file.to!)?.type === 'blob' &&
                tree.get(file.to!)?.mode === '100644' &&
                tree.get(file.to!)?.sha === file.attemptedSha,
        )
    )
        return { complete: true };
    const renamed = upstreamRename(job, tree);
    if (renamed) {
        if (renamed === job.to && !job.files.some((file) => file.attemptedSha)) return { complete: true };
        return { blocked: { reason: 'renamed', path: renamed } };
    }
    if (job.kind === 'rename' && job.folder && tree.has(job.to!))
        return { blocked: { reason: 'destination', path: job.to! } };
    const changes: StructuralFile[] = [];
    for (const file of job.files) {
        const source = tree.get(file.from);
        if (!source && job.kind === 'delete') continue;
        if (source?.type !== 'blob' || source.mode !== '100644' || source.sha !== file.sha)
            return {
                blocked: {
                    reason: 'source',
                    path: file.from,
                    ...(source?.type === 'blob' && source.mode === '100644' ? { sha: source.sha } : {}),
                },
            };
        if (file.to) {
            if (tree.has(file.to)) return { blocked: { reason: 'destination', path: file.to } };
            const parts = file.to.split('/');
            while (parts.length > 1) {
                parts.pop();
                const path = parts.join('/'),
                    entry = tree.get(path);
                if (entry && entry.type !== 'tree') return { blocked: { reason: 'destination', path } };
            }
        }
        changes.push(file);
    }
    return changes.length ? { complete: false, changes } : { complete: true };
}
