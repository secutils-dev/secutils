import { icon } from '../../../../shared/icons.js';
import { changeTable, readTable, resizeTable, type TableAction, type TableModel } from './table.ts';
import { setTableWidths } from './table-layout.ts';

interface Host { text(): string; enabled(): boolean; edit(start: number, end: number, text: string, anchor: HTMLElement): void; toast(text: string): void }
interface Selection { table: HTMLTableElement; model: TableModel; ordinal: number; row: number; column: number }
interface Restore { ordinal: number; row: number; column: number; epoch: number; focus?: string }
interface Gesture { selection: Selection; kind: 'resize' | 'row' | 'column'; pointer: number; x: number; y: number; clientX: number; clientY: number; time: number; widths: number[]; initial: number[] | null; target: number; active: boolean; handle: HTMLElement }

export function createTableEditor(article: HTMLElement, pane: HTMLElement, host: Host) {
    const toolbar = document.createElement('div');
    toolbar.className = 'su-table-tools'; toolbar.setAttribute('role', 'group'); toolbar.setAttribute('aria-label', 'Table actions'); toolbar.hidden = true;
    const rowGroup = document.createElement('div'), columnGroup = document.createElement('div');
    const rowLabel = document.createElement('span'), columnLabel = document.createElement('span');
    rowGroup.className = columnGroup.className = 'su-table-tools-group';
    rowGroup.append(rowLabel); columnGroup.append(columnLabel); toolbar.append(rowGroup, columnGroup); pane.append(toolbar);
    const line = document.createElement('div'); line.className = 'su-table-drop-line'; line.hidden = true; pane.append(line);
    const buttons = new Map<string, HTMLButtonElement>();
    const add = (group: HTMLElement, name: string, glyph: 'plus' | 'trash' | 'chevronRight' | 'drag' | 'refresh', rotate = 0) => {
        const button = document.createElement('button'); button.type = 'button'; button.className = 'btn btn-xs'; button.title = name; button.setAttribute('aria-label', name);
        button.innerHTML = icon(glyph); if (rotate) button.firstElementChild!.setAttribute('style', `transform:rotate(${rotate}deg)`);
        group.append(button); buttons.set(name, button); return button;
    };
    for (const [group, axis] of [[rowGroup, 'row'], [columnGroup, 'column']] as const) {
        for (const before of [true, false]) {
            const button = add(group, `Insert ${axis} ${axis === 'row' ? before ? 'above' : 'below' : before ? 'before' : 'after'}`, 'plus');
            button.innerHTML = icon('plus', 12) + icon('chevronRight', 10);
            button.lastElementChild!.setAttribute('style', `transform:rotate(${axis === 'row' ? before ? 270 : 90 : before ? 180 : 0}deg)`);
        }
        add(group, `Remove ${axis}`, 'trash');
        add(group, `Move ${axis} ${axis === 'row' ? 'up' : 'left'}`, 'chevronRight', axis === 'row' ? 270 : 180);
        add(group, `Move ${axis} ${axis === 'row' ? 'down' : 'right'}`, 'chevronRight', axis === 'row' ? 90 : 0);
        add(group, `Drag ${axis} to reorder`, 'drag').classList.add('su-table-grip');
    }
    add(columnGroup, 'Reset column widths', 'refresh');
    let selected: Selection | null = null, pending: Restore | null = null, gesture: Gesture | null = null, epoch = -1;
    let frame = 0, scrollFrame = 0;
    const grids = new WeakMap<HTMLTableElement, HTMLTableCellElement[][]>();
    const tabCells = new WeakMap<HTMLTableElement, HTMLTableCellElement>();
    const owned = (s = selected) => !!s && s.table.isConnected && s.model.source === host.text() && host.enabled();
    const cells = (table: HTMLTableElement) => grids.get(table) ?? [...table.rows].map((r) => [...r.cells]);
    const compatible = (table: HTMLTableElement, model: TableModel) => table.rows.length === model.rows.length + 1
        && [...table.rows].every((r) => r.cells.length === model.header.cells.length && [...r.cells].every((c) => c.colSpan === 1 && c.rowSpan === 1));
    const position = () => {
        if (!owned() || toolbar.hidden) return;
        const rect = selected!.table.getBoundingClientRect();
        toolbar.style.width = `${Math.min(360, window.innerWidth - 16)}px`;
        toolbar.style.left = `${Math.max(8, Math.min(rect.left, window.innerWidth - toolbar.offsetWidth - 8))}px`;
        const top = rect.top - toolbar.offsetHeight - 8;
        toolbar.style.top = `${Math.max(8, Math.min(top < 8 ? rect.top + 8 : top, window.innerHeight - toolbar.offsetHeight - 8))}px`;
    };
    const select = (table: HTMLTableElement, row: number, column: number, focus = false) => {
        const model = readTable(host.text(), { start: Number(table.dataset.tableStart), end: Number(table.dataset.tableEnd) });
        if (!model || !compatible(table, model) || !host.enabled()) { cancel(); return; }
        const ordinal = [...article.querySelectorAll('table[data-table-start]')].indexOf(table);
        selected?.table.classList.remove('su-table-active');
        if (selected) cells(selected.table)[selected.row]?.[selected.column]?.classList.remove('su-table-selected');
        selected = { table, model, ordinal, row: Math.min(row, model.rows.length), column: Math.min(column, model.header.cells.length - 1) };
        table.classList.add('su-table-active');
        const grid = cells(table);
        const previousTab = tabCells.get(table) ?? grid[0][0]; previousTab.tabIndex = -1;
        const cell = grid[selected.row]?.[selected.column];
        cell?.classList.add('su-table-selected'); if (cell) { cell.tabIndex = 0; tabCells.set(table, cell); }
        rowLabel.textContent = selected.row ? `Row ${selected.row}` : 'Header'; columnLabel.textContent = `Column ${selected.column + 1}`;
        for (const [name, button] of buttons) {
            button.disabled = name === 'Remove column' ? model.header.cells.length <= 1
                : name === 'Insert row above' ? !selected.row
                : name === 'Remove row' ? !selected.row
                : name === 'Drag row to reorder' ? !selected.row || model.rows.length < 2
                : name === 'Drag column to reorder' ? model.header.cells.length < 2
                : name === 'Move row up' ? selected.row <= 1
                : name === 'Move row down' ? !selected.row || selected.row >= model.rows.length
                : name === 'Move column left' ? selected.column === 0
                : name === 'Move column right' ? selected.column >= model.header.cells.length - 1
                : name === 'Reset column widths' ? !model.widths : false;
        }
        toolbar.hidden = false; position(); if (focus) cell?.focus({ preventScroll: true });
    };
    const commit = (s: Selection, text: string, row = s.row, column = s.column, focus?: string) => {
        if (!owned(s)) { cancel(); return; }
        if (text === s.model.source.slice(s.model.start, s.model.end)) return;
        pending = { ordinal: s.ordinal, row, column, epoch, focus };
        host.edit(s.model.start, s.model.end, text, s.table.closest<HTMLElement>('.su-block') ?? s.table);
    };
    const action = (name: string) => {
        if (!owned()) { cancel(); return; }
        const s = selected!;
        if (name === 'Reset column widths') { commit(s, resizeTable(s.model, null), s.row, s.column, name); return; }
        const axis = name.includes('row') ? 'row' : 'column';
        const index = axis === 'row' ? Math.max(0, s.row - 1) : s.column;
        const kind = name.startsWith('Insert') ? /above|before/.test(name) || (axis === 'row' && !s.row) ? 'before' : 'after' : name.startsWith('Remove') ? 'remove' : 'move';
        const dir = /up|left/.test(name) ? -1 : 1;
        const operation: TableAction = { axis, index, kind, to: index + dir };
        try {
            const next = changeTable(s.model, operation);
            const target = kind === 'before' ? index : kind === 'after' ? index + 1 : kind === 'move' ? index + dir : index;
            commit(s, next, axis === 'row' ? target + 1 : s.row, axis === 'column' ? target : s.column, name);
            host.toast(kind === 'move' ? `Moved ${axis}` : kind === 'remove' ? `Removed ${axis}` : `Inserted ${axis}`);
        } catch (error) { host.toast((error as Error).message); }
    };
    const measured = (s: Selection) => s.model.widths ?? [...s.table.rows[0].cells].map((c) => c.getBoundingClientRect().width / s.table.getBoundingClientRect().width);
    const resized = (g: Gesture, dx: number) => {
        const widths = [...g.widths], i = g.selection.column, pair = widths[i] + widths[i + 1];
        const width = g.selection.table.getBoundingClientRect().width;
        const minimum = Math.min(48 / width, pair / 3);
        widths[i] = Math.max(minimum, Math.min(pair - minimum, widths[i] + dx / width)); widths[i + 1] = pair - widths[i];
        return widths;
    };
    const begin = (event: PointerEvent, kind: Gesture['kind'], handle: HTMLElement) => {
        if (event.button !== 0 || !owned() || gesture) return;
        if (kind === 'row' && !selected!.row) return;
        event.preventDefault(); event.stopPropagation(); handle.focus({ preventScroll: true });
        gesture = { selection: selected!, kind, pointer: event.pointerId, x: event.clientX, y: event.clientY, clientX: event.clientX, clientY: event.clientY, time: performance.now(), widths: measured(selected!), initial: selected!.model.widths, target: kind === 'row' ? selected!.row - 1 : selected!.column, active: false, handle };
        handle.setPointerCapture(event.pointerId);
    };
    const updateDrop = (g: Gesture) => {
        const table = g.selection.table, from = g.kind === 'row' ? g.selection.row - 1 : g.selection.column;
        const entries = g.kind === 'row' ? [...table.tBodies[0].rows] : [...table.rows[0].cells];
        const remaining = entries.filter((_, i) => i !== from), coordinate = g.kind === 'row' ? g.clientY : g.clientX;
        const at = remaining.findIndex((el) => { const r = el.getBoundingClientRect(); return coordinate < (g.kind === 'row' ? r.top + r.height / 2 : r.left + r.width / 2); });
        g.target = at < 0 ? remaining.length : at;
        const r = (remaining[g.target] ?? remaining.at(-1) ?? entries[from]).getBoundingClientRect(), tableRect = table.getBoundingClientRect();
        const edge = g.target < remaining.length ? (g.kind === 'row' ? r.top : r.left) : (g.kind === 'row' ? r.bottom : r.right);
        const bounds = table.closest<HTMLElement>('.su-table-edit-scroll')!.getBoundingClientRect();
        const viewport = article.parentElement!.getBoundingClientRect();
        const left = Math.max(0, tableRect.left, bounds.left), right = Math.min(window.innerWidth, tableRect.right, bounds.right);
        const top = Math.max(0, tableRect.top, viewport.top), bottom = Math.min(window.innerHeight, tableRect.bottom, viewport.bottom);
        Object.assign(line.style, g.kind === 'row' ? { top: `${Math.max(top, Math.min(edge, bottom))}px`, left: `${left}px`, width: `${right - left}px`, height: '2px' }
            : { top: `${top}px`, left: `${Math.max(left, Math.min(edge, right))}px`, width: '2px', height: `${bottom - top}px` });
        line.hidden = false;
    };
    const scrollDrag = (time: number) => {
        const g = gesture;
        if (!g?.active || g.kind === 'resize' || !owned(g.selection)) return;
        const scroll = g.kind === 'column' ? g.selection.table.closest<HTMLElement>('.su-table-edit-scroll')! : article.parentElement!;
        const r = scroll.getBoundingClientRect(), point = g.kind === 'column' ? g.clientX : g.clientY;
        const low = g.kind === 'column' ? r.left : Math.max(0, r.top), high = g.kind === 'column' ? r.right : Math.min(window.innerHeight, r.bottom);
        const pressure = point < low + 32 ? -Math.min(1, (low + 32 - point) / 32) : point > high - 32 ? Math.min(1, (point - high + 32) / 32) : 0;
        const amount = pressure * 600 * Math.max(0, Math.min(time - g.time, 50)) / 1000;
        g.time = time;
        if (g.kind === 'column') scroll.scrollLeft += amount; else scroll.scrollTop += amount;
        if (pressure) updateDrop(g);
        scrollFrame = requestAnimationFrame(scrollDrag);
    };
    const move = (event: PointerEvent) => {
        const g = gesture;
        if (!g || event.pointerId !== g.pointer) return;
        if (!owned(g.selection)) { cancel(); return; }
        if (!g.active && Math.hypot(event.clientX - g.x, event.clientY - g.y) < 4) return;
        if (!g.active && g.kind !== 'resize') scrollFrame = requestAnimationFrame(scrollDrag);
        g.active = true; g.clientX = event.clientX; g.clientY = event.clientY; g.handle.classList.add('su-table-dragging');
        if (g.kind === 'resize') {
            const widths = resized(g, event.clientX - g.x);
            setTableWidths(g.selection.table, widths);
            g.handle.setAttribute('aria-valuenow', String(Math.round(widths[g.selection.column] * 100)));
            return;
        }
        updateDrop(g);
    };
    const finish = (event: PointerEvent) => {
        const g = gesture;
        if (!g || event.pointerId !== g.pointer) return;
        cancelAnimationFrame(scrollFrame); gesture = null; line.hidden = true; g.handle.classList.remove('su-table-dragging');
        if (g.handle.hasPointerCapture(event.pointerId)) g.handle.releasePointerCapture(event.pointerId);
        if (!g.active || !owned(g.selection)) return;
        try {
            if (g.kind === 'resize') commit(g.selection, resizeTable(g.selection.model, resized(g, event.clientX - g.x)));
            else {
                const s = g.selection, index = g.kind === 'row' ? s.row - 1 : s.column;
                if (index !== g.target) commit(s, changeTable(s.model, { axis: g.kind, index, kind: 'move', to: g.target }), g.kind === 'row' ? g.target + 1 : s.row, g.kind === 'column' ? g.target : s.column);
            }
        } catch (error) {
            if (g.kind === 'resize') setTableWidths(g.selection.table, g.initial);
            host.toast((error as Error).message);
        }
    };
    function cancel() {
        cancelAnimationFrame(scrollFrame);
        const g = gesture; gesture = null; line.hidden = true;
        if (g) {
            if (g.kind === 'resize' && g.selection.table.isConnected) setTableWidths(g.selection.table, g.initial);
            g.handle.classList.remove('su-table-dragging');
            if (g.handle.hasPointerCapture(g.pointer)) g.handle.releasePointerCapture(g.pointer);
        }
        selected?.table.classList.remove('su-table-active');
        if (selected) cells(selected.table)[selected.row]?.[selected.column]?.classList.remove('su-table-selected');
        selected = null; toolbar.hidden = true;
    }
    toolbar.addEventListener('click', (event) => {
        const button = (event.target as HTMLElement).closest('button');
        if (button && !button.disabled && !button.classList.contains('su-table-grip')) action(button.getAttribute('aria-label')!);
    });
    for (const axis of ['row', 'column'] as const) {
        const button = buttons.get(`Drag ${axis} to reorder`)!;
        button.title = `Drag to reorder, or use ${axis === 'row' ? 'up/down' : 'left/right'} arrow keys.`;
        button.setAttribute('aria-description', button.title);
        button.addEventListener('pointerdown', (e) => begin(e, axis, button));
        button.addEventListener('pointermove', move); button.addEventListener('pointerup', finish);
        button.addEventListener('pointercancel', cancel); button.addEventListener('lostpointercapture', () => { if (gesture) cancel(); });
        button.addEventListener('keydown', (event) => {
            const backwards = axis === 'row' ? 'ArrowUp' : 'ArrowLeft', forwards = axis === 'row' ? 'ArrowDown' : 'ArrowRight';
            if (![backwards, forwards].includes(event.key)) return;
            event.preventDefault(); event.stopPropagation();
            const name = `Move ${axis} ${axis === 'row' ? event.key === backwards ? 'up' : 'down' : event.key === backwards ? 'left' : 'right'}`;
            if (!buttons.get(name)!.disabled) action(name);
        });
    }
    article.addEventListener('click', (event) => {
        if (gesture || (event.target as HTMLElement).closest('a, input, button')) return;
        const cell = (event.target as HTMLElement).closest<HTMLTableCellElement>('th, td'), table = cell?.closest<HTMLTableElement>('table[data-table-start]');
        if (cell && table && !window.getSelection()?.toString()) select(table, (cell.parentElement as HTMLTableRowElement).rowIndex, cell.cellIndex);
        else cancel();
    });
    article.addEventListener('focusin', (event) => {
        const cell = (event.target as HTMLElement).closest<HTMLTableCellElement>('th, td'), table = cell?.closest<HTMLTableElement>('table[data-table-start]');
        if (cell && table && !gesture) select(table, (cell.parentElement as HTMLTableRowElement).rowIndex, cell.cellIndex);
    });
    article.addEventListener('keydown', (event) => {
        if ((event.target as HTMLElement).matches('button') || !owned()) return;
        const direction = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] }[event.key];
        if (!direction || event.ctrlKey || event.metaKey) return;
        event.preventDefault(); event.stopPropagation();
        select(selected!.table, Math.max(0, selected!.row + direction[0]), Math.max(0, selected!.column + direction[1]), true);
    });
    document.addEventListener('pointerdown', (event) => {
        if (!toolbar.contains(event.target as Node) && !article.contains(event.target as Node)) cancel();
    }, true);
    document.addEventListener('keydown', (event) => { if (event.key === 'Escape' && (gesture || !toolbar.hidden)) { event.preventDefault(); event.stopPropagation(); cancel(); } }, true);
    const schedulePosition = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(position); };
    document.addEventListener('scroll', schedulePosition, true); window.addEventListener('resize', schedulePosition);

    return {
        cancel,
        attach(source: string, nextEpoch: number) {
            const restore = pending; pending = null; cancel(); epoch = nextEpoch;
            for (const table of article.querySelectorAll<HTMLTableElement>('table[data-table-start]')) {
                const model = readTable(source, { start: Number(table.dataset.tableStart), end: Number(table.dataset.tableEnd) });
                if (!model || !compatible(table, model)) continue;
                let wrap = table.parentElement!;
                if (!wrap.matches('.mdp-table-wrap')) { wrap = document.createElement('div'); wrap.className = 'mdp-table-wrap'; table.before(wrap); wrap.append(table); }
                wrap.classList.add('su-table-edit-scroll'); wrap.style.setProperty('--table-columns', String(table.rows[0].cells.length));
                wrap.tabIndex = -1;
                const grid = cells(table);
                grids.set(table, grid);
                for (const row of grid) for (const cell of row) cell.tabIndex = -1;
                grid[0][0].tabIndex = 0;
                tabCells.set(table, grid[0][0]);
                for (const [column, th] of [...table.rows[0].cells].entries()) {
                    if (column === grid[0].length - 1) continue;
                    th.classList.add('su-table-header-cell');
                    const handle = document.createElement('button'); handle.type = 'button'; handle.className = 'su-table-resize';
                    handle.setAttribute('role', 'separator'); handle.setAttribute('aria-orientation', 'vertical'); handle.setAttribute('aria-label', `Resize column ${column + 1}`);
                    handle.setAttribute('aria-valuemin', '0'); handle.setAttribute('aria-valuemax', '100');
                    handle.setAttribute('aria-valuenow', String(Math.round((model.widths?.[column] ?? 1 / grid[0].length) * 100)));
                    handle.title = 'Drag to resize. Arrow keys adjust width.'; th.append(handle);
                    handle.addEventListener('pointerdown', (event) => { select(table, 0, column); begin(event, 'resize', handle); });
                    handle.addEventListener('pointermove', move); handle.addEventListener('pointerup', finish);
                    handle.addEventListener('pointercancel', cancel); handle.addEventListener('lostpointercapture', () => { if (gesture) cancel(); });
                    handle.addEventListener('keydown', (event) => {
                        if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) return;
                        event.preventDefault(); event.stopPropagation(); select(table, 0, column);
                        if (!owned()) return;
                        const s = selected!, widths = measured(s);
                        const g = { selection: s, widths } as Gesture;
                        commit(s, resizeTable(s.model, resized(g, (event.key === 'ArrowLeft' ? -1 : 1) * (event.shiftKey ? 30 : 10))), 0, column, `Resize column ${column + 1}`);
                    });
                }
            }
            if (restore && restore.epoch === nextEpoch) {
                const table = [...article.querySelectorAll<HTMLTableElement>('table[data-table-start]')][restore.ordinal];
                if (table) {
                    select(table, restore.row, restore.column, !restore.focus);
                    const button = restore.focus && (buttons.get(restore.focus) ?? [...table.querySelectorAll<HTMLElement>('.su-table-resize')].find((b) => b.getAttribute('aria-label') === restore.focus));
                    if (button && !(button as HTMLButtonElement).disabled) button.focus({ preventScroll: true });
                    else if (restore.focus && selected) cells(table)[selected.row]?.[selected.column]?.focus({ preventScroll: true });
                }
            }
        },
    };
}
