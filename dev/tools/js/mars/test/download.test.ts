import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

const source = readFileSync(new URL('../src/ui/ui.ts', import.meta.url), 'utf8');
const code = source.slice(source.indexOf('export function download('), source.indexOf('export const fileSafe')).replace(/^export /, '');
for (const modal of [false, true]) {
    test(`download anchors stay connected to the ${modal ? 'active dialog' : 'body'} until navigation completes`, () => {
        const events: string[] = [];
        let cleanup!: () => void;
        const anchor = { hidden: false, click() { assert.equal(this.hidden, true); assert.ok(events.includes(modal ? 'modal' : 'body')); events.push('click'); }, remove() { events.push('remove'); } };
        const body = { append: () => events.push('body') }, dialog = { append: () => events.push('modal') };
        const download = runInNewContext(`${transformSync(code, { loader: 'ts' }).code}\ndownload`, {
            Blob, document: { body, querySelector: () => modal ? dialog : null, createElement: () => anchor },
            URL: { createObjectURL: () => 'blob:test', revokeObjectURL: () => events.push('revoke') },
            setTimeout: (fn: () => void, delay: number) => { assert.ok(delay >= 1000); cleanup = fn; },
        });
        download('Report.md', '# Report', 'text/markdown');
        assert.deepEqual(events, [modal ? 'modal' : 'body', 'click']);
        cleanup(); assert.deepEqual(events.slice(-2), ['remove', 'revoke']);
    });
}
