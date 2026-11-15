// Durable enrollment intent for explicitly created or recovered nodes. Intent is saved with the
// local node, before any service request. Keys stay in side records, never ordinary exports.
import { ancestors, newId, type TreeNode } from '../tree/model.ts';
import { encode } from './protocol.ts';

/** One resumable registration. `id` is local; `root` and `document` are portable, retry-stable identities. */
export interface CreationJob {
    v: 1;
    id: string;
    collection: string;
    member: string;
    root: string;
    document: string;
    key: string;
    recovery?: true;
    blocked?: true;
}

/** Creates explicit enrollment intent under an editable Sync folder, including a still-pending parent. */
export function creationIntent(nodes: TreeNode[], node: TreeNode): CreationJob | undefined {
    if (node.connection) return;
    const chain = ancestors(nodes, node.id).reverse();
    const parent = chain.find((item) => item.connection || item.syncPending);
    if (!parent || parent.kind !== 'folder') return;
    const binding = parent.connection;
    if (
        binding &&
        (binding.kind !== 'sync' || binding.phase === 'ended' || !['owner', 'edit'].includes(binding.permission))
    )
        return;
    const target = binding ?? parent.syncPending!;
    const job: CreationJob = {
        v: 1,
        id: node.id,
        collection: target.collection,
        member: target.member,
        root: binding?.root ?? parent.syncPending?.root ?? binding!.document,
        document: newId(),
        key: encode(crypto.getRandomValues(new Uint8Array(32))),
    };
    node.syncPending = { collection: job.collection, member: job.member, root: job.root, phase: 'waiting' };
    return job;
}
