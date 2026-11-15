import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

test('failed detachment retains in-memory bindings and sessions, with no early side-record deletion', async () => {
    const source = readFileSync(new URL('../src/sync/manager/access.ts', import.meta.url), 'utf8');
    const start = source.indexOf('export async function disconnect(');
    const code = transformSync(source.slice(start, source.indexOf('\n/**', start)), { loader: 'ts' }).code.replace(
        /^export /gm,
        '',
    );
    const connection = { collection: 'collection', member: 'member', document: 'document' };
    const node = { id: 'local', connection: { ...connection }, originId: 'original' };
    const scope = { collection: 'collection', member: { grant: { member: 'member' } }, exclusions: [] };
    let destroyed = false;
    const sessions = new Map([
        [
            'local',
            {
                destroy: () => {
                    destroyed = true;
                },
            },
        ],
    ]);
    const failure = new Error('Storage failed');
    const disconnect = runInNewContext(`${code}\ndisconnect`, {
        app: { state: { nodes: [node] }, serializePersistence: (run: () => Promise<void>) => run(), node: () => node },
        db: {
            putNodes: async () => {
                throw failure;
            },
            deleteSide: () => assert.fail('Side data was deleted before the transaction'),
        },
        driverFor: () => ({ stop() {}, retry() {} }),
        saveScope: async () => {},
        sessions,
        newId: () => 'new-origin',
    });
    await assert.rejects(disconnect(scope, [node]), (error) => error === failure);
    assert.equal(destroyed, false);
    assert.equal(sessions.size, 1);
    assert.deepEqual(node.connection, connection);
    assert.equal(node.originId, 'original');
});
