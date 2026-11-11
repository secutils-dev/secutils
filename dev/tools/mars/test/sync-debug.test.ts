import assert from 'node:assert/strict';
import { test } from 'node:test';
import { safeError, SyncDebugLog } from '../src/sync/debug.ts';
import { featureFlags } from '../src/platform/flags.ts';
import { clearDebugReport, debugReport, registerDebugProvider } from '../src/platform/debug.ts';

test('diagnostics stay quiet by default while retaining a minimal failure for explicit export', () => {
    const output: unknown[] = [],
        log = new SyncDebugLog(
            () => false,
            (event) => output.push(event),
        );
    log.record('setup.stage', { stage: 'checking-selection' });
    assert.equal(log.snapshot().events.length, 0);
    log.record('setup.failed', { stage: 'saving-connections', code: 'orphan-replica' }, true);
    assert.equal(log.snapshot().events.length, 1);
    assert.deepEqual(output, []);
});

test('diagnostic exports use allowlists rather than redacting arbitrary objects after logging', () => {
    const output: unknown[] = [],
        log = new SyncDebugLog(
            () => true,
            (event) => output.push(event),
        );
    const secret = 'private-document-and-secret';
    log.record('setup.stage', {
        stage: 'registering-items',
        items: 54,
        item: 12,
        token: secret,
        body: secret,
        name: secret,
        url: `https://mars.test/#live=${secret}`,
        headers: { authorization: secret },
        error: secret,
        code: secret,
        field: secret,
    });
    log.record(secret, { items: 1 });
    const exported = JSON.stringify(log.snapshot());
    assert.equal(exported.includes(secret), false);
    assert.equal(JSON.stringify(output).includes(secret), false);
    assert.equal(log.snapshot().events.length, 1);
    assert.deepEqual(Object.keys(log.snapshot().events[0]).sort(), ['at', 'event', 'item', 'items', 'stage']);
});

test('unknown errors cannot export their messages, stacks, names, or sensitive code strings', () => {
    const secret = 'Sensitive document name and bearer token';
    const error = {
        name: secret,
        code: secret,
        message: secret,
        stack: `${secret}\n at private (https://mars.test/#live=${secret}:123:45)`,
    };
    assert.deepEqual(safeError(error), { error: 'Error', code: 'unexpected-error', line: 123, column: 45 });
    assert.deepEqual(safeError({ name: 'Error', message: 'Private:123:45', stack: 'Error: Private:123:45' }), {
        error: 'Error',
        code: 'unexpected-error',
    });
    assert.deepEqual(safeError({ name: 'ReplicaConflict', reason: 'metadata-changed' }), {
        error: 'ReplicaConflict',
        code: 'metadata-changed',
    });
    assert.deepEqual(safeError({ name: 'SyncError', code: 'service-unavailable', status: 503 }), {
        error: 'SyncError',
        code: 'service-unavailable',
        status: 503,
    });
});

test('diagnostic storage is bounded, snapshots are independent, and Clear removes old events', () => {
    const log = new SyncDebugLog(() => true);
    for (let item = 0; item < 600; item++) log.record('setup.stage', { stage: 'registering-items', item });
    assert.equal(log.snapshot().events.length, 500);
    assert.equal(log.snapshot().dropped, 100);
    log.snapshot().events.splice(0);
    assert.equal(log.snapshot().events.length, 500);
    log.clear();
    assert.equal(log.snapshot().events.length, 0);
    assert.equal(log.snapshot().dropped, 0);
});

test('an explicit debug-off query overrides a saved flag even when the browser refuses storage writes', async () => {
    const names = ['location', 'sessionStorage', 'localStorage'] as const;
    const descriptors = names.map((name) => Object.getOwnPropertyDescriptor(globalThis, name));
    const storage = {
        getItem: () => 'sync',
        removeItem: () => {
            throw new DOMException('Denied', 'SecurityError');
        },
    };
    try {
        Object.assign(globalThis, {
            location: new URL('https://mars.test/?debug=off'),
            sessionStorage: storage,
            localStorage: storage,
        });
        const module = await import(new URL('../src/sync/debug.ts?debug-off-test', import.meta.url).href);
        assert.equal(module.debugEnabled(), false);
        assert.equal(module.debugEnabled(), false);
    } finally {
        for (const [index, name] of names.entries()) {
            if (descriptors[index]) Object.defineProperty(globalThis, name, descriptors[index]!);
            else Reflect.deleteProperty(globalThis, name);
        }
    }
});

test('the generic report includes registered channels but never unknown flag scopes or provider errors', () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document');
    let cleared = 0;
    try {
        Object.assign(globalThis, { document: { hidden: false } });
        featureFlags.set('debug:sync,debug:private_document_token');
        registerDebugProvider('example', {
            snapshot: () => ({ count: 2 }),
            clear: () => {
                cleared++;
            },
        });
        registerDebugProvider('unavailable', {
            snapshot: () => {
                throw new Error('PRIVATE-PROVIDER-ERROR');
            },
            clear: () => {},
        });
        const raw = debugReport(),
            value = JSON.parse(raw);
        assert.deepEqual(value.flags, ['debug:sync']);
        assert.deepEqual(value.channels.example, { count: 2 });
        assert.deepEqual(value.channels.unavailable, { unavailable: true });
        assert.equal(raw.includes('private_document_token'), false);
        assert.equal(raw.includes('PRIVATE-PROVIDER-ERROR'), false);
        clearDebugReport();
        assert.equal(cleared, 1);
    } finally {
        featureFlags.clear();
        if (descriptor) Object.defineProperty(globalThis, 'document', descriptor);
        else Reflect.deleteProperty(globalThis, 'document');
    }
});
