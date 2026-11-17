import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

const source = readFileSync(new URL('../src/app.ts', import.meta.url), 'utf8');
const start = source.indexOf('const listeners =');
const code = transformSync(source.slice(start, source.indexOf('/** The node with this id', start)), {
    loader: 'ts',
    format: 'cjs',
}).code;
const events = () =>
    runInNewContext(`${code}\nmodule.exports`, {
        module: { exports: {} },
        setTimeout,
        clearTimeout,
        performance,
    }) as {
        onChange(fn: () => void): () => void;
        notify(): void;
        batch<T>(fn: () => Promise<T>): Promise<T>;
    };

test('app listeners remain synchronous and can unsubscribe without affecting other consumers', () => {
    const api = events();
    const calls: string[] = [];
    const stop = api.onChange(() => calls.push('temporary'));
    api.onChange(() => calls.push('persistent'));
    api.notify();
    assert.deepEqual(calls, ['temporary', 'persistent']);
    stop();
    stop();
    api.notify();
    assert.deepEqual(calls, ['temporary', 'persistent', 'persistent']);
});

test('a listener can retire itself during notification without skipping the next consumer', () => {
    const api = events();
    let temporary = 0,
        persistent = 0;
    const stop = api.onChange(() => {
        temporary++;
        stop();
    });
    api.onChange(() => persistent++);
    api.notify();
    api.notify();
    assert.equal(temporary, 1);
    assert.equal(persistent, 2);
});

test('a batch coalesces notifications, notifies once when it settles, and stays quiet without changes', async () => {
    const api = events();
    let calls = 0;
    api.onChange(() => calls++);
    await api.batch(async () => {
        for (let i = 0; i < 1000; i++) api.notify();
        await api.batch(async () => api.notify());
        assert.equal(calls, 0);
    });
    assert.equal(calls, 1);
    await assert.rejects(
        api.batch(async () => {
            api.notify();
            throw new Error('import failed');
        }),
    );
    assert.equal(calls, 2);
    await api.batch(async () => {});
    await new Promise((resolve) => setTimeout(resolve, 300));
    assert.equal(calls, 2);
    api.notify();
    assert.equal(calls, 3);
});
