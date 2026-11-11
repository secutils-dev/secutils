// Source ranges for rendered tables. This module wraps the `table`, `code`, and `html` methods of
// a marked renderer, so each located table carries `data-table-start` and `data-table-end`.
// The table editor reads those offsets. They are valid for the exact `source` string only.
// A document cannot supply these attributes: `safeRawHtml` removes `data-*` from raw HTML.
import type { Renderer } from 'marked';
import { locateTable } from './table.ts';

/**
 * Installs the table range mapping on `renderer` for one render of `source`.
 * Isolated MARS extension: the shared block/list mapper retains its existing behavior.
 *
 * Tables map in source order: a cursor moves past each located table, so two equal tables get
 * different ranges. Code and HTML tokens also move the cursor when their source lines are found,
 * which keeps table text inside them from matching a later table. A table that cannot be located
 * renders without the attributes.
 *
 * @returns A function that the block mapper calls before each top-level block with the source
 *   window `[start, end)` of that block. Its `skip(end, token)` moves the cursor to `end` for a
 *   code or HTML token that the mapper already located.
 */
export function installTableRanges(renderer: Renderer, source: string) {
    let cursor = 0,
        limit = source.length;
    const mapped = new WeakSet<object>();
    const table = renderer.table,
        code = renderer.code,
        html = renderer.html;
    const advance = (raw: string) => {
        const range = locateTable(source, raw, cursor, limit, true);
        if (range) cursor = range.end;
    };
    renderer.code = function (token) {
        if (!mapped.has(token)) advance(token.raw);
        return code.call(this, token);
    };
    renderer.html = function (token) {
        if (!mapped.has(token)) advance(token.raw);
        return html.call(this, token);
    };
    renderer.table = function (token) {
        const output = table.call(this, token);
        const range = locateTable(source, token.raw, cursor, limit);
        if (!range) return output;
        cursor = range.end;
        return output.replace(/^<table/, `<table data-table-start="${range.start}" data-table-end="${range.end}"`);
    };
    return Object.assign(
        (start: number, end: number) => {
            cursor = start;
            limit = end;
        },
        {
            skip(end: number, token: object) {
                cursor = Math.max(cursor, end);
                mapped.add(token);
            },
        },
    );
}
