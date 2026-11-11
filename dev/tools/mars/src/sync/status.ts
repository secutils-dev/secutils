// Transport activity belongs to this tab, not a saved tree snapshot. Only the
// durable ended marker survives reload; active status comes from the driver.
// A driver reports through `set`. A tree reload reads the state back through `restore`.
import type { TreeNode } from '../tree/model.ts';
import type { ConnectionStatus } from './transport.ts';

/** Transport state of each connection in this tab, by collection and member. It is not saved. */
export class ConnectionStates {
    private states = new Map<string, ConnectionStatus>();
    /** Records the state that a transport reports. */
    set(binding: { collection: string; member: string }, state: ConnectionStatus) {
        this.states.set(`${binding.collection}:${binding.member}`, state);
    }
    /**
     * Sets the phase of each connected node from the live state, or to `waiting` without a report. An `ended` phase
     * stays. It changes the given nodes and returns the same array.
     */
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
/** The instance of this tab. */
export const connectionStates = new ConnectionStates();
