// Browser-local attachment identity. A title is never used as a Git path.
import { ancestors, type TreeNode } from '../tree/model.ts';

/** Find the GitHub root for a row, without interpreting a local folder name as a remote path. */
export const githubRoot = (nodes: TreeNode[], node?: TreeNode): TreeNode | undefined =>
    node && (node.github ? node : ancestors(nodes, node.id).find((parent) => parent.github));
