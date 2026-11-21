// Browser-local attachment identity. A title is never used as a Git path.
import { ancestors, type TreeNode } from '../tree/model.ts';

/** The prototype is available on the dev deployment and local preview only. */
export const githubEnabled = () =>
    document.documentElement.hasAttribute('data-mars-dev') ||
    ['localhost', '127.0.0.1', '[::1]'].includes(location.hostname);

/** Find the GitHub root for a row, without interpreting a local folder name as a remote path. */
export const githubRoot = (nodes: TreeNode[], node?: TreeNode): TreeNode | undefined =>
    node && (node.github ? node : ancestors(nodes, node.id).find((parent) => parent.github));
