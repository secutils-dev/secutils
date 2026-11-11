// Transport activity belongs to this tab, not a saved tree snapshot. Only the
// durable ended marker survives reload; active status comes from the driver.
import type { TreeNode } from '../tree/model.ts';
import type { ConnectionStatus } from './transport.ts';

export class ConnectionStates {
    private states = new Map<string, ConnectionStatus>();
    set(binding: { collection: string; member: string }, state: ConnectionStatus) {
        this.states.set(`${binding.collection}:${binding.member}`, state);
    }
    restore(nodes: TreeNode[]) {
        for (const node of nodes)
            if (node.connection && node.connection.phase !== 'ended') {
                node.connection = {
                    ...node.connection,
                    phase: this.states.get(`${node.connection.collection}:${node.connection.member}`) ?? 'waiting',
                };
            }
        return nodes;
    }
}
export const connectionStates = new ConnectionStates();
