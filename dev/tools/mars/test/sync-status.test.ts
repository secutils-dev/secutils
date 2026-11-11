import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ConnectionStates } from '../src/sync/status.ts';
import type { TreeNode } from '../src/tree/model.ts';

const node = (phase: NonNullable<TreeNode['connection']>['phase'] = 'working', member = 'member'): TreeNode => ({
    id: crypto.randomUUID(),
    kind: 'doc',
    parentId: null,
    name: 'Private',
    createdAt: 1,
    updatedAt: 1,
    order: 0,
    connection: {
        collection: 'collection',
        document: crypto.randomUUID(),
        member,
        kind: 'sync',
        permission: 'owner',
        epoch: 1,
        phase,
    },
});
test('a saved working phase cannot resume an animation after reopening a tab', () => {
    const states = new ConnectionStates(),
        nodes = [node('working'), node('current'), node('offline'), node('attention')];
    states.restore(nodes);
    assert.ok(nodes.every((node) => node.connection!.phase === 'waiting'));
});
test('tree refreshes use the current driver state, not the old phase saved with two document records', () => {
    const states = new ConnectionStates(),
        one = node(),
        two = node(),
        original = structuredClone([one, two]);
    states.set(one.connection!, 'current');
    states.restore([one, two]);
    assert.equal(one.connection!.phase, 'current');
    const reloaded = states.restore(structuredClone(original));
    assert.ok(reloaded.every((node) => node.connection!.phase === 'current'));
    states.set(one.connection!, 'working');
    states.restore(reloaded);
    assert.ok(reloaded.every((node) => node.connection!.phase === 'working'));
});
test('ended connections remain ended, and another member does not inherit the owner’s activity', () => {
    const states = new ConnectionStates(),
        owner = node(),
        live = node('working', 'different-member'),
        ended = node('ended');
    states.set(owner.connection!, 'working');
    states.restore([owner, live, ended]);
    assert.equal(owner.connection!.phase, 'working');
    assert.equal(live.connection!.phase, 'waiting');
    assert.equal(ended.connection!.phase, 'ended');
});
