import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { marked } from 'marked';

const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
const md = readFileSync(new URL('../../../md.html', import.meta.url), 'utf8');
const section = (text: string, start: string, end: string) => text.slice(text.indexOf(start), text.indexOf(end, text.indexOf(start)));
const mapping = section(viewer, 'const LIST_ITEM_RE', '// Keep the review open');
const render = runInNewContext(`${mapping}\nrenderSourceBlocks`, { marked, escapeUnmatchedInlineHtml: () => {} });
const renumber = runInNewContext(`(${section(viewer, 'function renumberListItem', 'function moveOrderedItem')})`);
const itemRanges = (source: string) => [...render(source, new marked.Renderer()).matchAll(/<li\b[^>]*data-start="(\d+)" data-end="(\d+)"[^>]*>/g)]
    .map((match) => ({ start: Number(match[1]), text: source.slice(Number(match[1]), Number(match[2])) }));

test('source mapping and block controls match Markdown Preview', () => {
    for (const [start, end] of [
        ['const LIST_ITEM_RE', '// Keep the review open'],
        ['function renumberListItem', 'async function copyTarget'],
        ['// ---- Block handle', '// ---- Selection formatting toolbar'],
    ]) assert.equal(section(viewer, start, end), section(md, start, end));
});

test('nested multiline list items retain original source ranges and indentation', () => {
    const source = '1. Parent\n   1. First\n   2. Wrapped\n      continuation\n   3. Last\n2. Outside\n';
    const ranges = itemRanges(source);
    assert.equal(ranges.length, 5);
    assert.equal(ranges[2].text, '   2. Wrapped\n      continuation\n');
    assert.equal(ranges[3].text, '   3. Last');
    assert.equal(ranges[4].text, '2. Outside');
    assert.equal(ranges[2].start, source.indexOf('   2. Wrapped'));
});

test('list markers in code fences do not steal subsequent nested item ranges', () => {
    const source = '- Parent\n\n  ```md\n  - Child\n  ```\n\n  - Child\n  - Wrapped\n    continuation\n- Outside\n';
    const ranges = itemRanges(source);
    const child = ranges.find(({ text }) => text.trimStart().startsWith('- Child'))!;
    assert.equal(child.start, source.lastIndexOf('  - Child'));
    assert.equal(ranges.length, 4);
    const inline = '- ```md\n  - Child\n  ```\n\n  - Child\n- Outside\n';
    const actual = itemRanges(inline).find(({ text }) => text.trimStart().startsWith('- Child'))!;
    assert.equal(actual.start, inline.lastIndexOf('  - Child'));
});

test('nested tab-indented items map to original characters, not expanded token columns', () => {
    const source = '1. Parent\n\t1. Short\n\t2. Wrapped\n\t\tcontinuation\n\t3. Last\n2. Outside\n';
    const ranges = itemRanges(source);
    assert.equal(ranges.length, 5);
    assert.equal(ranges[2].text, '\t2. Wrapped\n\t\tcontinuation\n');
    assert.equal(ranges[2].start, source.indexOf('\t2. Wrapped'));
});

test('renumbering preserves nested text and code while adjusting continuation columns', () => {
    const source = '9. Parent\n   continuation\n   - child\n     nested\n\n   ```md\n   42. literal\n   ```';
    assert.equal(renumber(source, 10), '10. Parent\n    continuation\n    - child\n      nested\n\n    ```md\n    42. literal\n    ```');
    assert.equal(renumber('10) [x] Wrapped\n    more\n\n    Second paragraph.\nlazy words', 9),
        '9) [x] Wrapped\n   more\n\n   Second paragraph.\nlazy words');
    assert.equal(renumber('9.\tTabbed\n\tcontinuation', 10), '10.\tTabbed\n\tcontinuation');
    assert.equal(renumber('99. Item\n\tcontinuation', 100), '100. Item\n\t continuation');
    assert.equal(renumber('100. Item\n\t continuation', 99), '99. Item\n\tcontinuation');
    assert.equal(renumber('- Bullet\n  continuation', 2), '- Bullet\n  continuation');
});
