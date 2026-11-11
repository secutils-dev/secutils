// MARS-only table source operations. Layout travels in Markdown, not browser side data.
//
// This module owns the mapping between a GFM pipe table and its source text. It locates a table,
// parses it into a `TableModel`, and computes the text of an edit. It is pure (no DOM, no storage)
// and never changes the document. The caller splices the returned text into the source.
// - Every offset is a UTF-16 index into `TableModel.source`. A model is valid for that exact
//   string only. Read the table again after any source change.
// - Each operation states the range that its result replaces and the bytes that it keeps.
// - Column proportions live in an adjacent `<!-- su-table {"v":1,"cols":[...]} -->` comment.
//   The comment is document content, so its values are validated before use.
import { marked } from 'marked';
/** A half-open `[start, end)` range of source offsets. */
export interface TableRange {
    start: number;
    end: number;
}
/** A splice: replace `[start, end)` of the source with `text`. */
export interface TableEdit extends TableRange {
    text: string;
}
export type TableAlignment = 'left' | 'center' | 'right';
/**
 * One source line of a table. `raw` is the line without its line break, and `prefix` is its
 * indentation and blockquote markers. `newline` is its line break ('' when the range ends without
 * one). `cells` holds each cell's source text with its padding and without the outer pipes.
 */
interface Row {
    cells: string[];
    raw: string;
    prefix: string;
    newline: string;
}
/**
 * A parsed table. `[tableStart, end)` covers the table lines. `[start, end)` also covers the
 * `su-table` comment when one precedes the table, and equals the table range otherwise.
 * Whole-table edits replace `[start, end)`.
 */
export interface TableModel extends TableRange {
    tableStart: number;
    /** The complete document body that the offsets index into. */
    source: string;
    header: Row;
    divider: Row;
    /** Body rows. A short row is padded with empty cells to the header length. */
    rows: Row[];
    /** Column shares that sum to 1, or null without a valid `su-table` comment. */
    widths: number[] | null;
    /** Source text from `start` to `tableStart`: the comment line and an optional blank line, or ''. */
    marker: string;
    /** Line break of the header line (`\n` when it has none), used for inserted lines. */
    eol: string;
}
/**
 * A structural change for `changeTable`. `index` is zero-based: a column, or a body row (the
 * header is not a row here). `before` and `after` insert next to `index`, and `move` needs `to`.
 */
export type TableAction = {
    axis: 'row' | 'column';
    index: number;
    kind: 'before' | 'after' | 'remove' | 'move';
    to?: number;
};
/** Splits a line into its indentation and blockquote markers (group 1) and the rest (group 2). */
const PREFIX = /^([ \t]*(?:>[ \t]*)*)(.*)$/;
/** A complete `su-table` comment line without its prefix. Group 1 is the JSON metadata. */
const MARKER = /^<!--\s*su-table\s+([^\r\n]*?)\s*-->[ \t]*$/;

/**
 * Splits a document body into the source text of its top-level blocks, for review baselines.
 * Review baselines must group the same source span as the live table block, including its marker. A
 * `su-table` comment and the table (GFM or raw HTML) that follows it form one entry.
 */
export function tableReviewBlocks(source: string): string[] {
    const raws: string[] = [];
    let cursor = 0,
        markerStart = -1;
    for (const token of marked.lexer(source)) {
        if (token.type === 'space') continue;
        const start = source.indexOf(token.raw, cursor);
        if (start >= 0) cursor = start + token.raw.length;
        const table = token.type === 'table' || (token.type === 'html' && /^\s*<table(?:\s|>)/i.test(token.raw));
        if (table && markerStart >= 0 && start >= 0) raws[raws.length - 1] = source.slice(markerStart, cursor);
        else raws.push(token.raw);
        markerStart = token.type === 'html' && MARKER.test(token.raw.trim()) ? start : -1;
    }
    return raws;
}

/**
 * Validates column widths and scales them to shares that sum to 1. Returns null unless `value`
 * is an array of exactly `count` positive finite numbers, with 1 to 1,000 columns and no share
 * below 0.000001. The input can come from document content.
 */
export function normalizeWidths(value: unknown, count: number): number[] | null {
    if (
        !Array.isArray(value) ||
        value.length !== count ||
        !count ||
        count > 1000 ||
        value.some((n) => typeof n !== 'number' || !Number.isFinite(n) || n <= 0)
    )
        return null;
    const sum = value.reduce((a, b) => a + b, 0);
    if (!Number.isFinite(sum) || !sum) return null;
    const widths = value.map((n) => n / sum);
    return widths.some((n) => n < 0.000001) ? null : widths;
}
/**
 * Reads column shares from the JSON of a `su-table` comment. Accepts only `{"v":1,"cols":[...]}`
 * without other keys, for a table with `count` columns. Returns null for anything else.
 */
export function metadataWidths(text: string, count: number): number[] | null {
    try {
        const data = JSON.parse(text);
        return data?.v === 1 && Object.keys(data).every((key) => ['v', 'cols'].includes(key))
            ? normalizeWidths(data.cols, count)
            : null;
    } catch {
        return null;
    }
}

/**
 * Ranges of the cells in one table line, relative to `text` (the line without its prefix).
 * GFM pipes split cells unless escaped by an odd number of backslashes, including code spans.
 * A blank first or last span comes from an outer pipe and is not a cell.
 */
function cellSpans(text: string): TableRange[] {
    const cells: TableRange[] = [];
    let from = 0,
        slashes = 0;
    for (let i = 0; i < text.length; i++) {
        if (text[i] === '|' && slashes % 2 === 0) {
            cells.push({ start: from, end: i });
            from = i + 1;
        }
        slashes = text[i] === '\\' ? slashes + 1 : 0;
    }
    cells.push({ start: from, end: text.length });
    if (cells.length > 1 && !text.slice(cells[0].start, cells[0].end).trim()) cells.shift();
    if (cells.length > 1 && !text.slice(cells.at(-1)!.start).trim()) cells.pop();
    return cells;
}
/** Source text of each cell in one table line, with its padding and without the outer pipes. */
export const splitTableCells = (text: string): string[] =>
    cellSpans(text).map(({ start, end }) => text.slice(start, end));

/**
 * Source range of a cell's content without its padding. `line` counts source lines: 0 is the
 * header, 1 the divider, 2 the first body row. An empty cell yields an empty range at its middle.
 * Returns null when the source line has no such cell (an implicit cell of a short row).
 */
function contentRange(model: TableModel, line: number, column: number): TableRange | null {
    const lines = [model.header, model.divider, ...model.rows],
        row = lines[line];
    const span = cellSpans(row.raw.slice(row.prefix.length))[column];
    if (!span) return null;
    const offset =
        model.tableStart +
        lines.slice(0, line).reduce((sum, r) => sum + r.raw.length + r.newline.length, 0) +
        row.prefix.length;
    const raw = row.raw.slice(row.prefix.length + span.start, row.prefix.length + span.end);
    const start = span.start + (raw.trim() ? raw.length - raw.trimStart().length : Math.floor(raw.length / 2));
    return { start: offset + start, end: offset + (raw.trim() ? span.end - raw.length + raw.trimEnd().length : start) };
}
/**
 * Source range of the content of a cell, for selection formatting in Preview. `row` 0 is the
 * header and `row` n is body row n. Returns null for a cell outside the table and for an
 * implicit cell of a short row.
 */
export function tableCellRange(model: TableModel, row: number, column: number): TableRange | null {
    if (
        !Number.isInteger(row) ||
        row < 0 ||
        row > model.rows.length ||
        !Number.isInteger(column) ||
        column < 0 ||
        column >= model.header.cells.length
    )
        return null;
    return contentRange(model, row ? row + 1 : 0, column);
}

/**
 * Computes the splice that sets the content of one cell. The splice covers only the content of
 * that cell, so padding, the other cells, and the other lines keep their bytes. A line break in
 * `value` becomes `<br>`, and an unescaped pipe gains a backslash.
 *
 * An implicit cell of a short row has no source range. The splice then replaces that row after
 * its prefix with every cell written out.
 *
 * @param row 0 for the header, n for body row n.
 * @throws Error when the cell is outside the table.
 */
export function editTableCell(model: TableModel, row: number, column: number, value: string): TableEdit {
    if (
        !Number.isInteger(row) ||
        row < 0 ||
        row > model.rows.length ||
        !Number.isInteger(column) ||
        column < 0 ||
        column >= model.header.cells.length
    )
        throw new Error('Invalid table cell.');
    let text = '',
        slashes = 0;
    for (const char of value.replace(/\r\n?|\n/g, '<br>')) {
        text += char === '|' && slashes % 2 === 0 ? '\\|' : char;
        slashes = char === '\\' ? slashes + 1 : 0;
    }
    const line = row ? row + 1 : 0,
        range = contentRange(model, line, column);
    if (range) return { ...range, text };
    const lines = [model.header, model.divider, ...model.rows],
        target = lines[line];
    const start =
        model.tableStart +
        lines.slice(0, line).reduce((sum, r) => sum + r.raw.length + r.newline.length, 0) +
        target.prefix.length;
    const cells = [...target.cells];
    cells[column] = ` ${text} `;
    return { start, end: start + target.raw.length - target.prefix.length, text: `|${cells.join('|')}|` };
}

/** Alignment of a column from the colons of its divider cell. A divider without a trailing colon is `left`. */
export function columnAlignment(model: TableModel, column: number): TableAlignment {
    const value = model.divider.cells[column]?.trim() ?? '';
    return value.endsWith(':') ? (value.startsWith(':') ? 'center' : 'right') : 'left';
}

/**
 * Returns the replacement for `[model.start, model.end)` with a new alignment for one column.
 * Only the colons of that divider cell change. Its dashes, its padding, and every other byte stay.
 *
 * @throws Error for a column outside the table or an unknown alignment.
 */
export function alignTableColumn(model: TableModel, column: number, alignment: TableAlignment): string {
    if (
        !Number.isInteger(column) ||
        column < 0 ||
        column >= model.header.cells.length ||
        !['left', 'center', 'right'].includes(alignment)
    )
        throw new Error('Invalid column alignment.');
    const range = contentRange(model, 1, column)!;
    if (columnAlignment(model, column) === alignment) return model.source.slice(model.start, model.end);
    const dashes = model.divider.cells[column].trim().replaceAll(':', '');
    const text = `${alignment !== 'right' ? ':' : ''}${dashes}${alignment !== 'left' ? ':' : ''}`;
    return model.source.slice(model.start, range.start) + text + model.source.slice(range.end, model.end);
}

/**
 * Finds the source lines of a table token. For a table inside a blockquote or a list item,
 * marked strips the quote markers and the indentation from `token.raw`. The raw text then does not
 * occur verbatim in the source. Recover quoted/indented table tokens without letting a table's
 * source range include other prose.
 *
 * A match starts at the start of a source line at or after `from` and ends at or before `limit`.
 * Each line must end with the token line. Before it, only indentation and blockquote markers
 * are allowed, with the blockquote depth of the first line.
 *
 * @param raw `token.raw` of the table.
 * @param allowInlineList Also accept a list marker before the first line (`- | a | b |`).
 * @returns The range of whole lines, with the last line break only when `raw` ends with one,
 *   or null when no candidate matches.
 */
export function locateTable(
    source: string,
    raw: string,
    from = 0,
    limit = source.length,
    allowInlineList = false,
): TableRange | null {
    const lines = raw.replace(/\r\n/g, '\n').replace(/\n+$/, '').split('\n');
    if (lines.length < 2 || !lines[0].trim()) return null;
    let candidate = source.indexOf(lines[0].trimStart(), from);
    while (candidate >= 0 && candidate < limit) {
        const start = source.lastIndexOf('\n', candidate - 1) + 1;
        const quoteDepth = (source.slice(start, candidate).match(/>/g) ?? []).length;
        let end = start,
            matches = start >= from;
        for (const [index, expected] of lines.entries()) {
            const newline = source.indexOf('\n', end);
            const stop = newline < 0 ? source.length : newline;
            const line = source.slice(end, stop).replace(/\r$/, '');
            const content = expected.trimStart();
            const prefix = line.slice(0, line.length - content.length);
            const validPrefix =
                index === 0 && allowInlineList
                    ? /^[ \t]*(?:>[ \t]*)*(?:(?:[-+*]|\d{1,9}[.)])[ \t]+)?$/.test(prefix)
                    : /^[ \t]*(?:>[ \t]*)*$/.test(prefix);
            if (
                !line.endsWith(content) ||
                !validPrefix ||
                (content && (prefix.match(/>/g) ?? []).length !== quoteDepth) ||
                stop > limit
            ) {
                matches = false;
                break;
            }
            end = newline < 0 || (index === lines.length - 1 && !raw.endsWith('\n')) ? stop : newline + 1;
        }
        if (matches && end <= limit) return { start, end };
        candidate = source.indexOf(lines[0].trimStart(), candidate + 1);
    }
    return null;
}

/**
 * Parses the table lines in `range` (from `locateTable`) into a model. Returns null unless the
 * lines form a pipe table. A pipe table has a header, a divider of dashes with the same cell count,
 * and no body row with more cells than the header.
 *
 * A `su-table` comment on the line before the table belongs to it when both lines have the same
 * blockquote markers. One blank line can separate them. The model then starts at the comment
 * line. A comment with invalid metadata still belongs to the table and yields `widths: null`.
 */
export function readTable(source: string, range: TableRange): TableModel | null {
    const raw = source.slice(range.start, range.end);
    const lines = [...raw.matchAll(/([^\r\n]*)(\r?\n|$)/g)].filter((m) => m[0]);
    const parsed = lines.map((m): Row => {
        const [, prefix = '', text = ''] = m[1].match(PREFIX) ?? [];
        return { cells: splitTableCells(text), raw: m[1], prefix, newline: m[2] };
    });
    const [header, divider, ...rows] = parsed;
    if (
        !header ||
        !divider ||
        !header.cells.length ||
        header.cells.length !== divider.cells.length ||
        divider.cells.some((c) => !/^:?-+:?$/.test(c.trim())) ||
        rows.some((r) => r.cells.length > header.cells.length)
    )
        return null;
    // Short GFM rows have implicit empty cells. Never discard extra source cells hidden by a renderer.
    for (const row of rows) while (row.cells.length < header.cells.length) row.cells.push(' ');
    let start = range.start,
        marker = '',
        widths: number[] | null = null;
    const before = source.slice(0, start);
    const previous = before.match(/(?:^|\n)([^\r\n]*)(\r?\n)(?:[ \t]*(?:>[ \t]*)*\r?\n)?$/);
    if (previous) {
        const [, prefix = '', text = ''] = previous[1].match(PREFIX) ?? [];
        const metadata = text.match(MARKER);
        if (metadata && prefix.replace(/[ \t]/g, '') === header.prefix.replace(/[ \t]/g, '')) {
            start = before.length - previous[0].length + (previous[0].startsWith('\n') ? 1 : 0);
            marker = source.slice(start, range.start);
            widths = metadataWidths(metadata[1], header.cells.length);
        }
    }
    return {
        ...range,
        start,
        tableStart: range.start,
        source,
        header,
        divider,
        rows,
        marker,
        widths,
        eol: header.newline || '\n',
    };
}

/** A `su-table` comment line for `widths`, with the prefix of the header line. Shares keep 8 decimals. */
const markerText = (model: TableModel, widths: number[]) =>
    `${model.header.prefix}<!-- su-table ${JSON.stringify({ v: 1, cols: widths.map((n) => Number(n.toFixed(8))) })} -->${model.eol}`;
/**
 * Returns the replacement for `[model.start, model.end)` with new column widths. The table lines
 * keep their bytes. Only the `su-table` comment changes: `null` removes it, and an array writes
 * it directly above the header.
 *
 * @throws Error when `widths` fails `normalizeWidths` for the column count.
 */
export function resizeTable(model: TableModel, widths: number[] | null): string {
    if (widths === null) return model.source.slice(model.tableStart, model.end);
    const normalized = normalizeWidths(widths, model.header.cells.length);
    if (!normalized) throw new Error('Invalid table column widths.');
    return markerText(model, normalized) + model.source.slice(model.tableStart, model.end);
}

/**
 * Returns the replacement for `[model.start, model.end)` after a row or column insert, removal,
 * or move.
 *
 * - A row action keeps the `su-table` comment and the bytes of every remaining line. An inserted
 *   row has empty cells and the prefix of the divider line.
 * - A column action writes each line again as `prefix|cells|` with the original cell text. Cell
 *   Markdown and padding stay, and the outer pipes become explicit. Saved widths follow the
 *   columns: a move reorders them, a removal scales the rest to 1, and an insert gives the
 *   column an equal share.
 *
 * @throws Error for an index outside the table, a removal of the only column, or a move without a
 *   valid `to`. A row removal in a table without body rows also throws.
 */
export function changeTable(model: TableModel, action: TableAction): string {
    const { axis, index, kind, to } = action;
    const count = axis === 'column' ? model.header.cells.length : model.rows.length;
    if (!Number.isInteger(index) || index < 0 || (count ? index >= count : index !== 0))
        throw new Error('Invalid table selection.');
    if (kind === 'remove' && (!count || (axis === 'column' && count === 1)))
        throw new Error('Keep at least one table column.');
    if (kind === 'move' && (!Number.isInteger(to) || to! < 0 || to! >= count)) throw new Error('Invalid table move.');
    // An insert into a table without body rows uses index 0 and lands at position 0.
    const at = kind === 'before' ? index : kind === 'after' ? Math.min(count, index + 1) : index;
    const move = <T>(items: T[]) => {
        items.splice(to!, 0, ...items.splice(index, 1));
    };
    if (axis === 'row') {
        const rows = [...model.rows];
        if (kind === 'move') move(rows);
        else if (kind === 'remove') rows.splice(index, 1);
        else
            rows.splice(at, 0, {
                cells: Array(model.header.cells.length).fill(' '),
                prefix: model.divider.prefix,
                raw: `${model.divider.prefix}|${Array(model.header.cells.length).fill(' ').join('|')}|`,
                newline: model.eol,
            });
        const lines = [model.header, model.divider, ...rows];
        return (
            model.marker +
            lines
                .map(
                    (r, i) =>
                        r.raw +
                        // The result ends with a line break only when the original table range did.
                        (i === lines.length - 1
                            ? model.source.slice(model.tableStart, model.end).endsWith('\n')
                                ? model.eol
                                : ''
                            : r.newline || model.eol),
                )
                .join('')
        );
    }
    const lines = [model.header, model.divider, ...model.rows].map((row, rowIndex) => {
        const cells = [...row.cells];
        if (kind === 'move') move(cells);
        else if (kind === 'remove') cells.splice(index, 1);
        else cells.splice(at, 0, rowIndex === 1 ? ' --- ' : ' ');
        return `${row.prefix}|${cells.join('|')}|${row.newline}`;
    });
    let widths = model.widths && [...model.widths];
    if (widths) {
        if (kind === 'move') move(widths);
        else if (kind === 'remove') widths.splice(index, 1);
        else {
            widths = widths.map((w) => (w * count) / (count + 1));
            widths.splice(at, 0, 1 / (count + 1));
        }
        widths = normalizeWidths(widths, widths.length);
    }
    return (widths ? markerText(model, widths) : model.marker) + lines.join('');
}
