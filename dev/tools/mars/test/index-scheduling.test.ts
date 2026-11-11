import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

test('Sync status notifications cannot postpone indexing; text, name, and document-set changes still debounce', () => {
    const source = readFileSync(new URL('../src/search/embed.ts', import.meta.url), 'utf8');
    const code = transformSync(source.slice(source.indexOf('let timer: ReturnType<typeof setTimeout>')), {
        loader: 'ts',
    }).code;
    let listener = () => {},
        calls = 0,
        clears = 0,
        enabled = true;
    const nodes = [{ id: 'doc', kind: 'doc', name: 'Report', updatedAt: 1 }];
    runInNewContext(code, {
        app: { state: { nodes }, onChange: (fn: () => void) => (listener = fn) },
        enabledModel: () => (enabled ? 'fast' : null),
        reindex: () => {},
        setTimeout: () => ++calls,
        clearTimeout: () => clears++,
    });
    listener();
    assert.equal(calls, 1);
    for (let i = 0; i < 20; i++) listener();
    assert.equal(calls, 1);
    assert.equal(clears, 1);
    nodes[0].updatedAt++;
    listener();
    assert.equal(calls, 2);
    nodes[0].name = 'Renamed';
    listener();
    assert.equal(calls, 3);
    nodes.push({ id: 'second', kind: 'doc', name: 'New', updatedAt: 1 });
    listener();
    assert.equal(calls, 4);
    nodes.shift();
    listener();
    assert.equal(calls, 5);
    enabled = false;
    nodes[0].updatedAt++;
    listener();
    assert.equal(calls, 5);
});
