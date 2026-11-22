// Browser-local, case-sensitive .gitignore rules. The repository tree and working copies stay intact.
import ignore from 'ignore';
import type { TreeNode } from '../tree/model.ts';
import type { StructuralJob } from './structure.ts';

/** Maximum length of the rule editor, in UTF-16 code units. */
export const MAX_EXCLUSION_LENGTH = 32_768;

/** A predicate for repository-relative Git paths. Directory paths require a trailing slash. */
export function githubExclusion(rules?: string): (path: string) => boolean {
    if (!rules) return () => false;
    const matcher = ignore({ ignorecase: false }).add(rules);
    return (path) => ignore.isPathValid(path) && matcher.ignores(path);
}

/** Create one literal, root-anchored rule, including any special characters in the Git path. */
export const literalExclusion = (path: string, folder: boolean) =>
    `/${path.replace(/[\\*?\[\] ]/g, '\\$&')}${folder ? '/' : ''}`;

/** The first path that would put an explicit GitHub operation outside the included tree. */
export function excludedJobPath(job: StructuralJob, excludes: ReturnType<typeof githubExclusion>) {
    const suffix = job.folder ? '/' : '';
    return [
        job.from + suffix,
        ...(job.to ? [job.to + suffix] : []),
        ...job.files.flatMap(({ from, to }) => (to ? [from, to] : [from])),
    ].find(excludes);
}

/** Project the visible tree without discarding cached documents, edits, or their IDs. */
export function visibleGitHubNodes(nodes: TreeNode[]): TreeNode[] {
    const roots = nodes.filter((node) => node.github && node.githubExcludes);
    if (!roots.length) return nodes;
    const children = new Map<string, TreeNode[]>();
    for (const node of nodes) {
        if (!node.parentId) continue;
        const group = children.get(node.parentId);
        if (group) group.push(node);
        else children.set(node.parentId, [node]);
    }
    const hidden = new Set<string>();
    for (const root of roots) {
        const excludes = githubExclusion(root.githubExcludes);
        const visit = (node: TreeNode, prefix: string): boolean => {
            if (node.github) return true;
            const path = `${prefix}${node.name}/`;
            const branch = children.get(node.id) ?? [];
            let visibleChildren = false;
            for (const child of branch) if (visit(child, path)) visibleChildren = true;
            const excluded =
                node.kind === 'folder'
                    ? !visibleChildren && (branch.length > 0 || excludes(path))
                    : node.githubFile?.root === root.id && excludes(node.githubFile.path);
            if (!excluded || node.githubChange) return true;
            hidden.add(node.id);
            return false;
        };
        for (const child of children.get(root.id) ?? []) visit(child, '');
    }
    return nodes.filter((node) => !hidden.has(node.id));
}
