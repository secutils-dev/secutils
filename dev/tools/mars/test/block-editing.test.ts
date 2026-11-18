import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { marked } from 'marked';
import { installTableRanges } from '../src/doc/table-renderer.ts';
import { markdownDetails } from '../src/doc/markdown-details.ts';

marked.use({ extensions: [markdownDetails] });

const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
const section = (text: string, start: string, end: string) =>
    text.slice(text.indexOf(start), text.indexOf(end, text.indexOf(start)));
const mapping = section(viewer, 'const LIST_ITEM_RE', '// Keep the review open');
const render = runInNewContext(`${mapping}\nrenderSourceBlocks`, {
    marked,
    installTableRanges,
    escapeUnmatchedInlineHtml: () => {},
});
const renumber = runInNewContext(`(${section(viewer, 'function renumberListItem', 'function moveOrderedItem')})`);
const itemRanges = (source: string) =>
    [...render(source, new marked.Renderer()).matchAll(/<li\b[^>]*data-start="(\d+)" data-end="(\d+)"[^>]*>/g)].map(
        (match) => ({ start: Number(match[1]), text: source.slice(Number(match[1]), Number(match[2])) }),
    );

test('nested multiline list items retain original source ranges and indentation', () => {
    const source = '1. Parent\n   1. First\n   2. Wrapped\n      continuation\n   3. Last\n2. Outside\n';
    const ranges = itemRanges(source);
    assert.equal(ranges.length, 5);
    assert.equal(ranges[2].text, '   2. Wrapped\n      continuation\n');
    assert.equal(ranges[3].text, '   3. Last');
    assert.equal(ranges[4].text, '2. Outside');
    assert.equal(ranges[2].start, source.indexOf('   2. Wrapped'));
});

test('disclosures keep one source wrapper around all paragraphs and the matching end tag', () => {
    const source =
        '# Before\n\n<details>\n<summary>Title</summary>\n**First**.\n\n## Inside\n\nLast.\n</details>\n\nAfter.';
    const html = render(source, new marked.Renderer());
    const start = source.indexOf('<details>');
    const end = source.indexOf('</details>') + '</details>'.length;
    assert.match(html, new RegExp(`<div class="su-block" data-start="${start}" data-end="${end}"><details>`));
    assert.match(html, /<p>Last\.<\/p>\n<\/details><\/div><div class="su-block"[^>]*><p>After\.<\/p>/);
    assert.equal([...html.matchAll(/class="su-block"/g)].length, 3);
});

test('table and task ranges inside a disclosure ignore identical fenced examples', () => {
    const table = '| Name | State |\n| --- | --- |\n| Alice | Ready |';
    const source = `<details>\n<summary>Title</summary>\n\n\`\`\`md\n${table}\n- [ ] Task\n\`\`\`\n\n${table}\n\n- [ ] Task\n</details>`;
    const html = render(source, new marked.Renderer());
    const range = html.match(/data-table-start="(\d+)" data-table-end="(\d+)"/)!;
    assert.equal(Number(range[1]), source.lastIndexOf(table));
    assert.equal(source.slice(Number(range[1]), Number(range[2])).trimEnd(), table);
    const task = html.match(/data-check="(\d+)"/)!;
    assert.equal(Number(task[1]), source.lastIndexOf('[ ] Task'));
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

test('table ranges skip identical examples in inline list fences and stop before later fences', () => {
    const table = '| Name | State |\n| --- | --- |\n| Alice | Ready |';
    const quoted = (prefix: string) =>
        table
            .split('\n')
            .map((line) => `${prefix}${line}\n`)
            .join('');
    for (const prefix of ['', '> ']) {
        const example = `${prefix}- \`\`\`md\n${quoted(`${prefix}  `)}${prefix}  \`\`\`\n${prefix}\n`;
        const actual = quoted(`${prefix}  `);
        const source = example + actual + `${prefix}\n` + example;
        const ranges = [
            ...render(source, new marked.Renderer()).matchAll(/data-table-start="(\d+)" data-table-end="(\d+)"/g),
        ];
        assert.equal(ranges.length, 1);
        assert.equal(Number(ranges[0][1]), example.length);
        assert.equal(source.slice(Number(ranges[0][1]), Number(ranges[0][2])).trimEnd(), actual.trimEnd());
    }
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
    assert.equal(
        renumber(source, 10),
        '10. Parent\n    continuation\n    - child\n      nested\n\n    ```md\n    42. literal\n    ```',
    );
    assert.equal(
        renumber('10) [x] Wrapped\n    more\n\n    Second paragraph.\nlazy words', 9),
        '9) [x] Wrapped\n   more\n\n   Second paragraph.\nlazy words',
    );
    assert.equal(renumber('9.\tTabbed\n\tcontinuation', 10), '10.\tTabbed\n\tcontinuation');
    assert.equal(renumber('99. Item\n\tcontinuation', 100), '100. Item\n\t continuation');
    assert.equal(renumber('100. Item\n\t continuation', 99), '99. Item\n\tcontinuation');
    assert.equal(renumber('- Bullet\n  continuation', 2), '- Bullet\n  continuation');
});

test('list gaps retain adjacent targets, including nested and hidden items', () => {
    const hover = runInNewContext(`(${section(viewer, 'function listHoverTarget', 'function blockTargetAt')})`);
    const parent = {},
        one = {},
        two = {},
        three = {},
        hidden = {};
    const rows = [
        { item: parent, next: null, top: 0, bottom: 200 },
        { item: one, next: two, top: 10, bottom: 30 },
        { item: two, next: three, top: 40, bottom: 60 },
        { item: three, next: null, top: 80, bottom: 100 },
    ];
    assert.equal(hover(rows, 65, two), two);
    assert.equal(hover(rows, 75, two), two);
    assert.equal(hover(rows, 65, three), three);
    assert.equal(hover(rows, 65, one), two);
    assert.equal(hover(rows, 75, one), three);
    assert.equal(hover(rows, 70, null), two);
    assert.equal(hover(rows, 80, two), three);
    assert.equal(hover(rows, 45, three), two);
    assert.equal(
        hover(
            [rows[0], rows[1], { ...rows[2], next: hidden }, { item: hidden, next: three, top: 0, bottom: 0 }, rows[3]],
            70,
            two,
        ),
        two,
    );
});
