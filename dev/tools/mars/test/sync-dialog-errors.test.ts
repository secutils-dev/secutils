import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import { safeError } from '../src/sync/debug.ts';
import { LIMITS, SyncError } from '../src/sync/protocol.ts';

const source = readFileSync(new URL('../src/sync/manager/dialogs.ts', import.meta.url), 'utf8');
const start = source.indexOf('export const syncError =');
const code = transformSync(source.slice(start, source.indexOf('/**\n * Markup for an action button', start)), {
    loader: 'ts',
    format: 'cjs',
}).code;
const message = runInNewContext(`${code}\nmodule.exports.syncError`, {
    module: { exports: {} },
    safeError,
    LIMITS,
}) as (error: unknown) => string;

test('status error messages retain fixed-code and string-code compatibility', () => {
    assert.match(message('document-limit'), /1,000 documents and folders/);
    assert.match(message(new SyncError('sync-storage-full', 507)), /Sync storage is full/);
    assert.match(message(new Error('binding-missing')), /lost the saved connection/);
});

test('an unknown service code cannot select an inherited property or expose raw error text', () => {
    for (const code of ['constructor', '__proto__', 'toString', 'hasOwnProperty', 'PRIVATE DOCUMENT AND TOKEN']) {
        assert.equal(message(new SyncError(code)), 'Could not finish. Your document is still here. Try again.');
        assert.equal(message(code), 'Could not finish. Your document is still here. Try again.');
    }
});
