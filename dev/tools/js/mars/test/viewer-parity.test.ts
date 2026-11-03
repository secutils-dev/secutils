import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

test('Mermaid navigation and styles match Markdown Preview', () => {
    const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
    const md = readFileSync(new URL('../../../md.html', import.meta.url), 'utf8');
    const css = readFileSync(new URL('../src/doc/doc.css', import.meta.url), 'utf8');
    const helper = (s: string) => s.slice(s.indexOf('function initMermaids('), s.indexOf('// Maps use SVG;'));
    const styles = (s: string) => s.slice(s.indexOf('/* Mermaid:'), s.indexOf('.su-diagram {'));
    assert.ok(helper(viewer).length > 100);
    assert.equal(helper(viewer), helper(md));
    assert.equal(styles(css), styles(md));
});
