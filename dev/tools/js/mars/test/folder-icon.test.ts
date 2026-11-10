import assert from 'node:assert/strict';
import { test } from 'node:test';
import { folderIcon, icon, ICONS } from '../src/ui/icons.ts';

test('default folder paths share one SVG viewport with a centered 6px star in each state', () => {
    for (const open of [false, true]) {
        const svg = folderIcon(open, true);
        assert.equal(svg.match(/<svg\b/g)?.length, 1);
        assert.ok(svg.includes(ICONS[open ? 'folderOpen' : 'folder']));
        assert.ok(svg.includes(ICONS.starFill));
        assert.ok(svg.includes(`transform="translate(5 ${open ? 7 : 6}) scale(0.375)"`));
        assert.match(svg, /viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"/);
        assert.doesNotMatch(svg, /position:|<span/);
    }
});
test('ordinary folder icons remain byte-identical to their existing EUI glyphs', () => {
    assert.equal(folderIcon(false), icon('folder'));
    assert.equal(folderIcon(true), icon('folderOpen'));
});
