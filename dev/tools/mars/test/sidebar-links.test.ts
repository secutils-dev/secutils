import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

const sidebar = readFileSync(new URL('../src/sidebar/sidebar.ts', import.meta.url), 'utf8');
// The sidebar module needs a DOM at import, so the test evaluates the one pure declaration from its source.
const start = sidebar.indexOf('const docLink =');
const source = sidebar.slice(start, sidebar.indexOf('\n\n', start));
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

const linkFor = (pathname: string, currentId: string | null, temporary = false) =>
    runInNewContext(`${transformSync(source, { loader: 'ts' }).code}\ndocLink`, {
        location: { pathname },
        app: { state: { currentId } },
        db: { isTemporaryStorage: () => temporary },
        esc,
        encodeURIComponent,
    });

test('document anchors use the local mount and ID, with no shared snapshot or forced target', () => {
    const link = linkFor('/review/', 'doc-id')({ id: 'doc-id', name: 'Review' }, '<span>Review</span>');
    assert.match(link, /href="\/review\/#doc=doc-id"/);
    assert.match(link, /aria-current="page"/);
    assert.match(link, /tabindex="-1" draggable="false"/);
    assert.ok(!link.includes('target='));
    assert.ok(!link.includes('&md='));
});

test('temporary documents are buttons, not broken browser-local links', () => {
    const link = linkFor('/review/', 'temp', true)({ id: 'temp', name: 'Temporary' }, '<span>Temporary</span>');
    assert.match(link, /^<button type="button" class="tree-link"/);
    assert.ok(!link.includes('href='));
    assert.ok(link.endsWith('</button>'));
});

test('document anchors escape labels and URL identifiers and mark only the current document', () => {
    const link = linkFor('/review&notes/', 'other')(
        { id: 'id&value', name: 'A "name" <with> & symbols' },
        '<span>Safe content</span>',
    );
    assert.match(link, /href="\/review&amp;notes\/#doc=id%26value"/);
    assert.match(link, /aria-label="A &quot;name&quot; &lt;with&gt; &amp; symbols"/);
    assert.ok(!link.includes('aria-current='));
    assert.ok(link.endsWith('<span>Safe content</span></a>'));
});
