import assert from 'node:assert/strict';
import test from 'node:test';
import { marked } from 'marked';
import { alignTableColumn, columnAlignment, editTableCell, changeTable, locateTable, metadataWidths, normalizeWidths, readTable, resizeTable, splitTableCells, tableReviewBlocks, tableCellRange } from '../src/doc/table.ts';
import { installTableRanges } from '../src/doc/table-renderer.ts';
import { tableMetadataHtml } from '../src/doc/table-layout.ts';
import { decodeState, encodeState, openSealed, sealBundle } from '../src/share/codec.ts';

const TABLE = '| A | B |\n| :--- | ---: |\n| one | two |\n| three | four |\n';
const model = (source = TABLE, table = TABLE) => readTable(source, { start: source.indexOf(table), end: source.indexOf(table) + table.length })!;
const replace = (source: string, table: ReturnType<typeof model>, next: string) => source.slice(0, table.start) + next + source.slice(table.end);
const cellEdit = (source: string, parsed: ReturnType<typeof model>, row: number, column: number, value: string) => {
    const edit = editTableCell(parsed, row, column, value);
    return source.slice(0, edit.start) + edit.text + source.slice(edit.end);
};

test('connected table layout carriers contain validated numbers only, never raw HTML or styles', () => {
    assert.equal(tableMetadataHtml('<!-- su-table {"v":1,"cols":[2,8]} -->\n'), '<span data-su-table-layout="0.2,0.8" hidden></span>');
    for (const source of [
        '<!-- unrelated comment -->',
        '<!-- su-table {"v":1,"cols":[]} -->',
        '<!-- su-table {"v":1,"cols":[0,1]} -->',
        '<!-- su-table {"v":1,"cols":[-1,2]} -->',
        '<!-- su-table {"v":1,"cols":["width:99%",1]} -->',
        '<!-- su-table {"v":1,"cols":[1,2],"style":"color:red"} -->',
        '<!-- su-table {"v":1,"cols":[1,2]} --><script>alert(1)</script>',
        '<span data-su-table-layout="0.2,0.8" hidden></span>',
    ]) assert.equal(tableMetadataHtml(source), null);
});

test('formatting ranges identify the exact cell despite repeated words in links, quotes, and metadata', () => {
    for (const prefix of ['', '> ', '> > ']) {
        const table = '| [Header](https://example.test/alpha/words) | Value |\n| --- | --- |\n| **alpha words** | alpha words |\n'.split('\n').filter(Boolean).map((line) => prefix + line + '\r\n').join('');
        const source = 'Before.\r\n\r\n' + prefix + '<!-- su-table {"v":1,"cols":[0.4,0.6]} -->\r\n' + table;
        const parsed = model(source, table), first = tableCellRange(parsed, 1, 0)!, second = tableCellRange(parsed, 1, 1)!;
        assert.equal(source.slice(first.start, first.end), '**alpha words**');
        assert.equal(source.slice(second.start, second.end), 'alpha words');
        assert.ok(first.start > source.indexOf('https://'));
        assert.equal(tableCellRange(parsed, -1, 0), null); assert.equal(tableCellRange(parsed, 1, 9), null);
    }
});

test('cell edits splice only the selected content, retaining padding, quotes, CRLF, metadata, and review bytes', () => {
    for (const prefix of ['', '> ', '> > ']) {
        const table = TABLE.split('\n').filter(Boolean).map((line) => `${prefix}${line}\r\n`).join('');
        const source = `Before.\r\n\r\n${prefix}<!-- su-table { "v":1,"cols":[0.25,0.75] } -->\r\n${table}\r\n<!-- su-comments\r\n[{"id":"review","quote":"four"}]\r\n-->\r\nAfter.`;
        const parsed = model(source, table);
        assert.equal(cellEdit(source, parsed, 1, 0, '**changed**'), source.replace(' one ', ' **changed** '));
        assert.equal(cellEdit(source, parsed, 0, 1, 'Header'), source.replace('| A | B |', '| A | Header |'));
        assert.equal(cellEdit(source, parsed, 2, 1, ''), source.replace(' four ', '  '));
        assert.equal(cellEdit(source, parsed, 1, 1, 'literal | pipe\nnext `a\\|b`'), source.replace(' two ', ' literal \\| pipe<br>next `a\\|b` '));
    }
});

test('cell edits retain unwrapped rows, support implicit empty cells, and reject invalid selections', () => {
    const table = ' A | B | C \n --- | --- | --- \n one | two \n';
    const parsed = model(table, table);
    assert.equal(cellEdit(table, parsed, 1, 0, 'new'), table.replace(' one ', ' new '));
    assert.equal(cellEdit(table, parsed, 1, 2, 'new'), ' A | B | C \n --- | --- | --- \n |one | two | new |\n');
    for (const [row, column] of [[-1, 0], [2, 0], [1, 3], [0, -1], [0.5, 0]]) assert.throws(() => editTableCell(parsed, row, column, 'invalid'));
});

test('column alignment updates only divider colons and preserves hyphens, cells, widths, and line endings', () => {
    const table = ' A | B \r\n  :-----  |   ------: \r\n one | two \r\n';
    const marker = '<!-- su-table { "v":1,"cols":[0.25,0.75] } -->\r\n';
    const source = marker + table, parsed = model(source, table);
    assert.equal(columnAlignment(parsed, 0), 'left'); assert.equal(columnAlignment(parsed, 1), 'right');
    assert.equal(alignTableColumn(parsed, 1, 'center'), source.replace('------:', ':------:'));
    assert.equal(alignTableColumn(parsed, 1, 'left'), source.replace('------:', ':------'));
    assert.equal(alignTableColumn(parsed, 0, 'right'), source.replace(':-----', '-----:'));
    assert.equal(alignTableColumn(parsed, 0, 'left'), source);
    assert.throws(() => alignTableColumn(parsed, 2, 'left'));
});

test('column metadata accepts proportional weights and rejects malformed or mismatched values', () => {
    assert.deepEqual(normalizeWidths([20, 80], 2), [0.2, 0.8]);
    assert.deepEqual(metadataWidths('{"v":1,"cols":[0.25,0.75]}', 2), [0.25, 0.75]);
    for (const value of ['broken', 'null', '{"v":2,"cols":[1,1]}', '{"v":1,"cols":[1]}', '{"v":1,"cols":[0,1]}', '{"v":1,"cols":["1",1]}', '{"v":1,"cols":[1,1],"rows":[]}', '{"v":1,"cols":[1e308,1e308]}']) assert.equal(metadataWidths(value, 2), null);
    for (const widths of [[NaN, 1], [Infinity, 1], [-1, 1], [1e-20, 1]]) assert.equal(normalizeWidths(widths, 2), null);
});

test('table cell splitting retains escaped pipes, backslashes, code, and inline Markdown', () => {
    assert.deepEqual(splitTableCells('| a\\|b | `x\\|y` | **bold** |'), [' a\\|b ', ' `x\\|y` ', ' **bold** ']);
    assert.deepEqual(splitTableCells('a\\\\| b'), ['a\\\\', ' b']);
    assert.deepEqual(splitTableCells('| | |'), [' ', ' ']);
    assert.deepEqual(splitTableCells(' A | B '), [' A ', ' B ']);
});

test('resizing only changes the adjacent metadata and retains surrounding Markdown/review bytes', () => {
    const table = TABLE.replaceAll('\n', '\r\n');
    const source = `---\r\ntitle: Exact\r\n---\r\n\r\n${table}\r\n<!-- su-comments\r\n[ { "id":"t", "quote":"one", "comments":[{"body":"Review"}] } ]\r\n-->\r\n`;
    const parsed = model(source, table);
    const next = replace(source, parsed, resizeTable(parsed, [0.3, 0.7]));
    assert.equal(next, source.replace(table, `<!-- su-table {"v":1,"cols":[0.3,0.7]} -->\r\n${table}`));
    const reopened = model(next, table);
    assert.deepEqual(reopened.widths, [0.3, 0.7]);
    assert.equal(replace(next, reopened, resizeTable(reopened, null)), source);
});

test('a marker separated by one blank line remains attached and does not capture other prose', () => {
    const marker = '<!-- su-table { "v": 1, "cols": [0.2, 0.8] } -->\n\n';
    const source = `Before.\n\n${marker}${TABLE}\nAfter.\n`;
    const parsed = model(source);
    assert.equal(parsed.marker, marker);
    assert.equal(parsed.start, source.indexOf(marker));
    assert.deepEqual(parsed.widths, [0.2, 0.8]);
    const unrelated = `<!-- su-table {"v":1,"cols":[1,1]} -->\n\nOther text.\n\n${TABLE}`;
    assert.equal(model(unrelated).widths, null);
    assert.equal(model(unrelated).start, unrelated.indexOf(TABLE));
});

test('row insertion, removal, and movement retain header, alignment, and existing metadata bytes', () => {
    const marker = '<!-- su-table { "v":1, "cols":[0.25,0.75] } -->\n';
    const parsed = model(marker + TABLE);
    assert.equal(changeTable(parsed, { axis: 'row', index: 0, kind: 'move', to: 1 }), marker + TABLE.replace('| one | two |\n| three | four |', '| three | four |\n| one | two |'));
    assert.equal(changeTable(parsed, { axis: 'row', index: 0, kind: 'remove' }), marker + TABLE.replace('| one | two |\n', ''));
    assert.equal(changeTable(parsed, { axis: 'row', index: 0, kind: 'before' }), marker + TABLE.replace('| one | two |', '| | |\n| one | two |'));
    const empty = '| A | B |\n| --- | --- |';
    assert.equal(changeTable(model(empty, empty), { axis: 'row', index: 0, kind: 'after' }), `${empty}\n| | |`);
});

test('column moves carry cell Markdown, alignment, and widths together', () => {
    const table = TABLE.replace('one', '**one**').replace('two', '`two\\|literal`');
    const parsed = model(`<!-- su-table {"v":1,"cols":[0.25,0.75]} -->\n${table}`, table);
    const changed = changeTable(parsed, { axis: 'column', index: 0, kind: 'move', to: 1 });
    assert.match(changed, /"cols":\[0.75,0.25\]/);
    assert.match(changed, /\| B \| A \|\n\| ---: \| :--- \|/);
    assert.match(changed, /\| `two\\\|literal` \| \*\*one\*\* \|/);
});

test('column insertion and deletion keep proportions normalized and protect the last column', () => {
    const parsed = model(`<!-- su-table {"v":1,"cols":[0.25,0.75]} -->\n${TABLE}`);
    const inserted = changeTable(parsed, { axis: 'column', index: 0, kind: 'after' });
    assert.match(inserted, /\| A \| \| B \|/);
    const table = inserted.slice(inserted.indexOf('\n') + 1);
    const next = model(inserted, table);
    assert.equal(next.header.cells.length, 3);
    assert.ok(Math.abs(next.widths!.reduce((a, b) => a + b, 0) - 1) < 1e-8);
    const removed = changeTable(parsed, { axis: 'column', index: 0, kind: 'remove' });
    assert.match(removed, /"cols":\[1\]/);
    const single = removed.slice(removed.indexOf('\n') + 1);
    assert.throws(() => changeTable(model(removed, single), { axis: 'column', index: 0, kind: 'remove' }));
});

test('quoted tables retain their quote prefixes when resized or structurally edited', () => {
    const quoted = TABLE.split('\n').filter(Boolean).map((line) => `> ${line}\n`).join('');
    const source = `> <!-- su-table {"v":1,"cols":[0.4,0.6]} -->\n${quoted}`;
    const range = locateTable(source, TABLE)!;
    assert.equal(source.slice(range.start, range.end), quoted);
    const parsed = readTable(source, range)!;
    assert.deepEqual(parsed.widths, [0.4, 0.6]);
    assert.match(resizeTable(parsed, [0.3, 0.7]), /^> <!-- su-table/);
    const edited = changeTable(parsed, { axis: 'column', index: 0, kind: 'after' });
    assert.ok(edited.split('\n').filter(Boolean).every((line) => line.startsWith('> ')));
});

test('nested quote metadata stays attached across an explicitly quoted blank line', () => {
    const table = TABLE.split('\n').filter(Boolean).map((line) => `> > ${line}\n`).join('');
    const marker = '> > <!-- su-table {"v":1,"cols":[0.25,0.75]} -->\n> >\n';
    const parsed = model(marker + table, table);
    assert.equal(parsed.marker, marker);
    assert.deepEqual(parsed.widths, [0.25, 0.75]);
    assert.equal((resizeTable(parsed, [0.4, 0.6]).match(/su-table/g) ?? []).length, 1);
});

test('source table ranges skip fenced examples and locate repeated and nested tables independently', () => {
    const source = `\`\`\`md\n${TABLE}\`\`\`\n\n${TABLE}\n> ${TABLE.replaceAll('\n', '\n> ').trimEnd().replace(/>$/, '')}`;
    const renderer = new marked.Renderer();
    installTableRanges(renderer, source)(0, source.length);
    const output = marked.parse(source, { renderer }) as string;
    const ranges = [...output.matchAll(/data-table-start="(\d+)" data-table-end="(\d+)"/g)];
    assert.equal(ranges.length, 2);
    assert.equal(Number(ranges[0][1]), source.indexOf(TABLE, source.indexOf('```', 3) + 3));
    assert.ok(source.slice(Number(ranges[1][1]), Number(ranges[1][2])).startsWith('> | A'));
});

test('table ranges honor marked tokens that omit the final newline before later prose', () => {
    const source = `# Title\n\n${TABLE}\nLater prose.\n`;
    const token = marked.lexer(source).find((t) => t.type === 'table')!;
    const start = source.indexOf(token.raw);
    const range = locateTable(source, token.raw, start, start + token.raw.length)!;
    assert.equal(source.slice(range.start, range.end), token.raw);
});

test('literal quote characters inside a fenced example cannot steal a later table range', () => {
    const source = `> \`\`\`md\n> > literal quoted example\n${TABLE.split('\n').filter(Boolean).map((line) => `> ${line}\n`).join('')}> \`\`\`\n>\n${TABLE.split('\n').filter(Boolean).map((line) => `> ${line}\n`).join('')}`;
    const renderer = new marked.Renderer();
    installTableRanges(renderer, source)(0, source.length);
    const output = marked.parse(source, { renderer }) as string;
    const range = output.match(/data-table-start="(\d+)"/)!;
    assert.equal(Number(range[1]), source.lastIndexOf('> | A'));
});

test('table layout survives full-state encoding and encrypted short-link bundles', async () => {
    const text = `<!-- su-table {"v":1,"cols":[0.2,0.8]} -->\n${TABLE}`;
    assert.equal(await decodeState(await encodeState(text)), text);
    const bundle = { v: 1 as const, kind: 'doc' as const, name: 'Table', docs: [{ path: [], name: 'Table', originId: 'table', text }] };
    const sealed = await sealBundle(bundle);
    assert.deepEqual(await openSealed(sealed.secret, sealed.blob), bundle);
});

test('review baselines group adjacent layout metadata with exactly its table source span', () => {
    const marker = '<!-- su-table {"v":1,"cols":[0.25,0.75]} -->\n\n';
    const source = `# Title\n\n${marker}${TABLE}\nLater prose.\n`;
    const blocks = tableReviewBlocks(source);
    assert.equal(blocks.length, 3);
    assert.equal(blocks[1], marker + TABLE.trimEnd());
    assert.equal(tableReviewBlocks(`# Title\n\n${TABLE}\nLater prose.\n`).length, 3);
});
