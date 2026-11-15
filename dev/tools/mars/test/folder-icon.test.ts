import assert from 'node:assert/strict';
import { test } from 'node:test';
import { folderIcon, documentIcon, icon, ICONS } from '../src/ui/icons.ts';

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
test('synced folders and documents have one SVG viewport and no separate positioned overlay', () => {
    for (const svg of [folderIcon(false, false, true), folderIcon(true, false, true), documentIcon(true)]) {
        assert.equal(svg.match(/<svg\b/g)?.length, 1);
        assert.match(svg, /class="tree-sync-mark"/);
        assert.ok(svg.includes(ICONS.refresh));
        assert.doesNotMatch(svg, /<span|position:|<use/);
    }
    assert.equal(documentIcon(), icon('document'));
    assert.equal(folderIcon(false, true, true), folderIcon(false, true));
    assert.equal(folderIcon(true, true, true), folderIcon(true, true));
});
test('ordinary folder icons remain byte-identical to their existing EUI glyphs', () => {
    assert.equal(folderIcon(false), icon('folder'));
    assert.equal(folderIcon(true), icon('folderOpen'));
});
