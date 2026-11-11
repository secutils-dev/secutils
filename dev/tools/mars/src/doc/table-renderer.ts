import type { Renderer } from 'marked';
import { locateTable } from './table.ts';

// Isolated MARS extension: the shared block/list mapper retains its existing behavior.
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
