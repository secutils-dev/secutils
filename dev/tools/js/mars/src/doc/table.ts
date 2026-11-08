// MARS-only table source operations. Layout travels in Markdown, not browser side data.
import { marked } from 'marked';
export interface TableRange { start: number; end: number }
export interface TableEdit extends TableRange { text: string }
export type TableAlignment = 'left' | 'center' | 'right';
interface Row { cells: string[]; raw: string; prefix: string; newline: string }
export interface TableModel extends TableRange {
    tableStart: number;
    source: string;
    header: Row;
    divider: Row;
    rows: Row[];
    widths: number[] | null;
    marker: string;
    eol: string;
}
export type TableAction = { axis: 'row' | 'column'; index: number; kind: 'before' | 'after' | 'remove' | 'move'; to?: number };
const PREFIX = /^([ \t]*(?:>[ \t]*)*)(.*)$/;
const MARKER = /^<!--\s*su-table\s+([^\r\n]*?)\s*-->[ \t]*$/;

// Review baselines must group the same source span as the live table block, including its marker.
export function tableReviewBlocks(source: string): string[] {
    const raws: string[] = [];
    let cursor = 0, markerStart = -1;
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

export function normalizeWidths(value: unknown, count: number): number[] | null {
    if (!Array.isArray(value) || value.length !== count || !count || count > 1000
        || value.some((n) => typeof n !== 'number' || !Number.isFinite(n) || n <= 0)) return null;
    const sum = value.reduce((a, b) => a + b, 0);
    if (!Number.isFinite(sum) || !sum) return null;
    const widths = value.map((n) => n / sum);
    return widths.some((n) => n < 0.000001) ? null : widths;
}
export function metadataWidths(text: string, count: number): number[] | null {
    try {
        const data = JSON.parse(text);
        return data?.v === 1 && Object.keys(data).every((key) => ['v', 'cols'].includes(key)) ? normalizeWidths(data.cols, count) : null;
    } catch { return null; }
}

// GFM pipes split cells unless escaped by an odd number of backslashes, including code spans.
function cellSpans(text: string): TableRange[] {
    const cells: TableRange[] = [];
    let from = 0, slashes = 0;
    for (let i = 0; i < text.length; i++) {
        if (text[i] === '|' && slashes % 2 === 0) { cells.push({ start: from, end: i }); from = i + 1; }
        slashes = text[i] === '\\' ? slashes + 1 : 0;
    }
    cells.push({ start: from, end: text.length });
    if (cells.length > 1 && !text.slice(cells[0].start, cells[0].end).trim()) cells.shift();
    if (cells.length > 1 && !text.slice(cells.at(-1)!.start).trim()) cells.pop();
    return cells;
}
export const splitTableCells = (text: string): string[] => cellSpans(text).map(({ start, end }) => text.slice(start, end));

function contentRange(model: TableModel, line: number, column: number): TableRange | null {
    const lines = [model.header, model.divider, ...model.rows], row = lines[line];
    const span = cellSpans(row.raw.slice(row.prefix.length))[column];
    if (!span) return null;
    const offset = model.tableStart + lines.slice(0, line).reduce((sum, r) => sum + r.raw.length + r.newline.length, 0) + row.prefix.length;
    const raw = row.raw.slice(row.prefix.length + span.start, row.prefix.length + span.end);
    const start = span.start + (raw.trim() ? raw.length - raw.trimStart().length : Math.floor(raw.length / 2));
    return { start: offset + start, end: offset + (raw.trim() ? span.end - raw.length + raw.trimEnd().length : start) };
}
export function tableCellRange(model: TableModel, row: number, column: number): TableRange | null {
    if (!Number.isInteger(row) || row < 0 || row > model.rows.length || !Number.isInteger(column) || column < 0 || column >= model.header.cells.length) return null;
    return contentRange(model, row ? row + 1 : 0, column);
}

export function editTableCell(model: TableModel, row: number, column: number, value: string): TableEdit {
    if (!Number.isInteger(row) || row < 0 || row > model.rows.length || !Number.isInteger(column) || column < 0 || column >= model.header.cells.length) throw new Error('Invalid table cell.');
    let text = '', slashes = 0;
    for (const char of value.replace(/\r\n?|\n/g, '<br>')) {
        text += char === '|' && slashes % 2 === 0 ? '\\|' : char;
        slashes = char === '\\' ? slashes + 1 : 0;
    }
    const line = row ? row + 1 : 0, range = contentRange(model, line, column);
    if (range) return { ...range, text };
    const lines = [model.header, model.divider, ...model.rows], target = lines[line];
    const start = model.tableStart + lines.slice(0, line).reduce((sum, r) => sum + r.raw.length + r.newline.length, 0) + target.prefix.length;
    const cells = [...target.cells]; cells[column] = ` ${text} `;
    return { start, end: start + target.raw.length - target.prefix.length, text: `|${cells.join('|')}|` };
}

export function columnAlignment(model: TableModel, column: number): TableAlignment {
    const value = model.divider.cells[column]?.trim() ?? '';
    return value.endsWith(':') ? value.startsWith(':') ? 'center' : 'right' : 'left';
}

export function alignTableColumn(model: TableModel, column: number, alignment: TableAlignment): string {
    if (!Number.isInteger(column) || column < 0 || column >= model.header.cells.length || !['left', 'center', 'right'].includes(alignment)) throw new Error('Invalid column alignment.');
    const range = contentRange(model, 1, column)!;
    if (columnAlignment(model, column) === alignment) return model.source.slice(model.start, model.end);
    const dashes = model.divider.cells[column].trim().replaceAll(':', '');
    const text = `${alignment !== 'right' ? ':' : ''}${dashes}${alignment !== 'left' ? ':' : ''}`;
    return model.source.slice(model.start, range.start) + text + model.source.slice(range.end, model.end);
}

// Recover quoted/indented table tokens without letting a table's source range include other prose.
export function locateTable(source: string, raw: string, from = 0, limit = source.length, allowInlineList = false): TableRange | null {
    const lines = raw.replace(/\r\n/g, '\n').replace(/\n+$/, '').split('\n');
    if (lines.length < 2 || !lines[0].trim()) return null;
    let candidate = source.indexOf(lines[0].trimStart(), from);
    while (candidate >= 0 && candidate < limit) {
        const start = source.lastIndexOf('\n', candidate - 1) + 1;
        const quoteDepth = (source.slice(start, candidate).match(/>/g) ?? []).length;
        let end = start, matches = start >= from;
        for (const [index, expected] of lines.entries()) {
            const newline = source.indexOf('\n', end);
            const stop = newline < 0 ? source.length : newline;
            const line = source.slice(end, stop).replace(/\r$/, '');
            const content = expected.trimStart();
            const prefix = line.slice(0, line.length - content.length);
            const validPrefix = index === 0 && allowInlineList
                ? /^[ \t]*(?:>[ \t]*)*(?:(?:[-+*]|\d{1,9}[.)])[ \t]+)?$/.test(prefix)
                : /^[ \t]*(?:>[ \t]*)*$/.test(prefix);
            if (!line.endsWith(content) || !validPrefix
                || (content && (prefix.match(/>/g) ?? []).length !== quoteDepth) || stop > limit) { matches = false; break; }
            end = newline < 0 || (index === lines.length - 1 && !raw.endsWith('\n')) ? stop : newline + 1;
        }
        if (matches && end <= limit) return { start, end };
        candidate = source.indexOf(lines[0].trimStart(), candidate + 1);
    }
    return null;
}

export function readTable(source: string, range: TableRange): TableModel | null {
    const raw = source.slice(range.start, range.end);
    const lines = [...raw.matchAll(/([^\r\n]*)(\r?\n|$)/g)].filter((m) => m[0]);
    const parsed = lines.map((m): Row => {
        const [, prefix = '', text = ''] = m[1].match(PREFIX) ?? [];
        return { cells: splitTableCells(text), raw: m[1], prefix, newline: m[2] };
    });
    const [header, divider, ...rows] = parsed;
    if (!header || !divider || !header.cells.length || header.cells.length !== divider.cells.length
        || divider.cells.some((c) => !/^:?-+:?$/.test(c.trim())) || rows.some((r) => r.cells.length > header.cells.length)) return null;
    // Short GFM rows have implicit empty cells. Never discard extra source cells hidden by a renderer.
    for (const row of rows) while (row.cells.length < header.cells.length) row.cells.push(' ');
    let start = range.start, marker = '', widths: number[] | null = null;
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
    return { ...range, start, tableStart: range.start, source, header, divider, rows, marker, widths, eol: header.newline || '\n' };
}

const markerText = (model: TableModel, widths: number[]) => `${model.header.prefix}<!-- su-table ${JSON.stringify({ v: 1, cols: widths.map((n) => Number(n.toFixed(8))) })} -->${model.eol}`;
export function resizeTable(model: TableModel, widths: number[] | null): string {
    if (widths === null) return model.source.slice(model.tableStart, model.end);
    const normalized = normalizeWidths(widths, model.header.cells.length);
    if (!normalized) throw new Error('Invalid table column widths.');
    return markerText(model, normalized) + model.source.slice(model.tableStart, model.end);
}

export function changeTable(model: TableModel, action: TableAction): string {
    const { axis, index, kind, to } = action;
    const count = axis === 'column' ? model.header.cells.length : model.rows.length;
    if (!Number.isInteger(index) || index < 0 || (count ? index >= count : index !== 0)) throw new Error('Invalid table selection.');
    if (kind === 'remove' && (!count || (axis === 'column' && count === 1))) throw new Error('Keep at least one table column.');
    if (kind === 'move' && (!Number.isInteger(to) || to! < 0 || to! >= count)) throw new Error('Invalid table move.');
    const at = kind === 'before' ? index : kind === 'after' ? Math.min(count, index + 1) : index;
    const move = <T>(items: T[]) => { items.splice(to!, 0, ...items.splice(index, 1)); };
    if (axis === 'row') {
        const rows = [...model.rows];
        if (kind === 'move') move(rows);
        else if (kind === 'remove') rows.splice(index, 1);
        else rows.splice(at, 0, { cells: Array(model.header.cells.length).fill(' '), prefix: model.divider.prefix, raw: `${model.divider.prefix}|${Array(model.header.cells.length).fill(' ').join('|')}|`, newline: model.eol });
        const lines = [model.header, model.divider, ...rows];
        return model.marker + lines.map((r, i) => r.raw + (i === lines.length - 1 ? (model.source.slice(model.tableStart, model.end).endsWith('\n') ? model.eol : '') : r.newline || model.eol)).join('');
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
        else { widths = widths.map((w) => w * count / (count + 1)); widths.splice(at, 0, 1 / (count + 1)); }
        widths = normalizeWidths(widths, widths.length);
    }
    return (widths ? markerText(model, widths) : model.marker) + lines.join('');
}
