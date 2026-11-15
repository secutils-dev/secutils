import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

test('pagehide gates enrollment until pageshow even before document.hidden changes', () => {
    const source = readFileSync(new URL('../src/sync/manager/creation.ts', import.meta.url), 'utf8');
    const code = transformSync(source.slice(source.indexOf('const wake =')), { loader: 'ts' }).code;
    const events = new Map<string, () => void>();
    let resumes = 0,
        aborted = 0;
    const api = runInNewContext(
        `let active = true;
        const available = () => active && !document.hidden && navigator.onLine;
        ${code}\n({available})`,
        {
            document: { hidden: false, addEventListener: (name: string, fn: () => void) => events.set(name, fn) },
            navigator: { onLine: true },
            window: { addEventListener: (name: string, fn: () => void) => events.set(name, fn) },
            resumeCreations: () => resumes++,
            controller: { abort: () => aborted++ },
            timer: undefined,
            clearTimeout: () => {},
        },
    );
    events.get('pagehide')!();
    assert.equal(api.available(), false);
    events.get('online')!();
    events.get('visibilitychange')!();
    assert.equal(resumes, 0);
    assert.ok(aborted > 0);
    events.get('pageshow')!();
    assert.equal(api.available(), true);
    assert.equal(resumes, 1);
});
