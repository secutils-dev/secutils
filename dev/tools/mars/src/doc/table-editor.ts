// Table controls in the live preview: cell selection, the cell editor, the row and column menus,
// drag reorder, and column resize. MARS-only.
//
// This module owns the controls and their DOM. It does not own the document: every change is a
// splice from `table.ts` that goes to `host.edit`, and the next render calls `attach`.
// - A selection is valid only while its model was read from the current source (`owned`).
//   Each action checks that first and cancels the selection otherwise.
// - A table is editable only when its rendered shape equals its source model (`compatible`).
// - The viewer calls `cancel` before a view change and an export capture, and `flush` when it
//   commits pending edits. Both apply an open cell draft first.
import { icon } from '../../../shared/icons.js';
import {
    alignTableColumn,
    columnAlignment,
    editTableCell,
    changeTable,
    readTable,
    resizeTable,
    type TableAction,
    type TableAlignment,
    type TableModel,
} from './table.ts';
import { setTableWidths } from './table-layout.ts';

/** What the editor needs from the viewer. */
interface Host {
    /** The current document body. Table offsets index into this string. */
    text(): string;
    /** True while table edits are allowed, for example in Preview with a render of the current text. */
    enabled(): boolean;
    /** Replaces `[start, end)` of the body with `text` and renders again. `anchor` keeps the scroll position. */
    edit(start: number, end: number, text: string, anchor: HTMLElement): void;
    toast(text: string): void;
    /** Runs before a cell editor opens, to close the other preview controls. */
    startCell(): void;
    /** True while a cell draft must stay open, because a remote change conflicts with it. */
    blocked?(): boolean;
}
/**
 * The selected cell. `row` 0 is the header and `row` n is body row n. `ordinal` is the index of
 * the table among the mapped tables of the article, which identifies it after a render.
 */
interface Selection {
    table: HTMLTableElement;
    model: TableModel;
    ordinal: number;
    row: number;
    column: number;
}
/**
 * A selection to restore after the next render. `epoch` is the document epoch of the request:
 * a render of another document drops it. `focus` names the control that gets the focus back.
 * `passive` restores the selection without moving the focus.
 */
interface Restore {
    ordinal: number;
    row: number;
    column: number;
    epoch: number;
    focus?: string;
    passive?: boolean;
}
/**
 * An open cell editor. `content` holds the rendered children of the cell during the edit, and
 * `original` is the trimmed source text of the cell when the editor opened.
 */
interface CellEditor {
    selection: Selection;
    cell: HTMLTableCellElement;
    form: HTMLDivElement;
    input: HTMLTextAreaElement;
    content: DocumentFragment;
    original: string;
}
/**
 * A pointer drag on a resize handle or a grip. `x`/`y` is the start point and `clientX`/`clientY`
 * the latest point, in viewport pixels. `time` is the timestamp of the last auto-scroll frame.
 * `widths` are the column shares at the start, and `initial` the saved widths for a cancel.
 * `target` is the drop index among the other rows or columns. `active` turns true after the
 * pointer moves 4 pixels, so a click on a grip still opens its menu.
 */
interface Gesture {
    selection: Selection;
    kind: 'resize' | 'row' | 'column';
    pointer: number;
    x: number;
    y: number;
    clientX: number;
    clientY: number;
    time: number;
    widths: number[];
    initial: number[] | null;
    target: number;
    active: boolean;
    handle: HTMLElement;
}

/**
 * Creates the table controls for one preview. Appends a toolbar and a drop line to `pane`, and
 * adds listeners to `article`, the document, and the window for the lifetime of the page.
 *
 * @param article The preview element that contains the rendered tables.
 * @param pane The element that hosts the floating controls.
 * @returns The controller that the viewer drives:
 *   - `attach(source, epoch)`: call after each preview render of `source`. Makes the mapped
 *     tables editable and restores the selection, the focus, or a running drag.
 *   - `flush()`: applies an open cell draft. Returns true when that produced a source edit.
 *   - `cancel()`: applies an open cell draft, aborts a drag, and clears the selection.
 *   - `editing()`: true while a cell editor is open.
 *   - `draft()`: the open cell draft as a splice of the source that the editor opened on, or null.
 *   - `discardDraft()`: closes the cell editor without an edit.
 */
export function createTableEditor(article: HTMLElement, pane: HTMLElement, host: Host) {
    const toolbar = document.createElement('div');
    toolbar.className = 'su-table-tools';
    toolbar.setAttribute('role', 'group');
    toolbar.setAttribute('aria-label', 'Table actions');
    toolbar.hidden = true;
    const rowGroup = document.createElement('div'),
        columnGroup = document.createElement('div');
    const rowLabel = document.createElement('span'),
        columnLabel = document.createElement('span');
    const panels = { row: rowGroup, column: columnGroup };
    const grips = {} as Record<'row' | 'column', HTMLButtonElement>;
    for (const axis of ['row', 'column'] as const) {
        const panel = panels[axis],
            grip = document.createElement('button');
        panel.className = 'su-table-axis-menu';
        panel.popover = 'auto';
        panel.id = `su-table-${axis}-${crypto.randomUUID()}`;
        panel.setAttribute('role', 'group');
        panel.setAttribute('aria-label', `${axis === 'row' ? 'Row' : 'Column'} actions`);
        grip.type = 'button';
        grip.className = `su-table-grip su-table-${axis}-grip`;
        grip.setAttribute('aria-label', `${axis === 'row' ? 'Row' : 'Column'} actions`);
        grip.setAttribute('popovertarget', panel.id);
        grip.innerHTML = `<span class="su-table-grip-face" aria-hidden="true"><span data-menu>${icon('boxesVertical', 12)}</span><span data-drag>${icon('drag', 12)}</span></span>`;
        grips[axis] = grip;
        toolbar.append(grip);
    }
    rowGroup.append(rowLabel);
    columnGroup.append(columnLabel);
    toolbar.append(rowGroup, columnGroup);
    pane.append(toolbar);
    const line = document.createElement('div');
    line.className = 'su-table-drop-line';
    line.hidden = true;
    pane.append(line);
    // Controls by their accessible label. The label also names the action and the focus to restore.
    const buttons = new Map<string, HTMLButtonElement | HTMLInputElement>();
    const add = (
        group: HTMLElement,
        name: string,
        glyph: 'plus' | 'trash' | 'chevronRight' | 'drag' | 'refresh',
        rotate = 0,
    ) => {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'btn btn-xs';
        button.title = name;
        button.setAttribute('aria-label', name);
        button.innerHTML = `${icon(glyph)}<span>${name}</span>`;
        if (rotate) button.firstElementChild!.setAttribute('style', `transform:rotate(${rotate}deg)`);
        group.append(button);
        buttons.set(name, button);
        return button;
    };
    for (const [group, axis] of [
        [rowGroup, 'row'],
        [columnGroup, 'column'],
    ] as const) {
        for (const before of [true, false]) {
            const button = add(
                group,
                `Insert ${axis} ${axis === 'row' ? (before ? 'above' : 'below') : before ? 'before' : 'after'}`,
                'plus',
            );
            button.innerHTML = `${icon('plus', 12)}<span>${button.getAttribute('aria-label')}</span>`;
        }
        add(group, `Remove ${axis}`, 'trash');
        add(group, `Move ${axis} ${axis === 'row' ? 'up' : 'left'}`, 'chevronRight', axis === 'row' ? 270 : 180);
        add(group, `Move ${axis} ${axis === 'row' ? 'down' : 'right'}`, 'chevronRight', axis === 'row' ? 90 : 0);
        buttons.set(`Drag ${axis} to reorder`, grips[axis]);
    }
    const alignment = document.createElement('fieldset'),
        legend = document.createElement('legend');
    alignment.className = 'su-table-alignment';
    legend.className = 'sr-only';
    legend.textContent = 'Column alignment';
    alignment.append(legend);
    const alignmentName = `su-table-align-${crypto.randomUUID()}`;
    const alignments = new Map<TableAlignment, HTMLInputElement>();
    const alignmentGlyphs = { left: 'textAlignLeft', center: 'textAlignCenter', right: 'textAlignRight' } as const;
    for (const value of ['left', 'center', 'right'] as const) {
        const label = document.createElement('label'),
            input = document.createElement('input');
        input.type = 'radio';
        input.name = alignmentName;
        input.value = value;
        input.setAttribute('aria-label', `Align column ${value}`);
        input.title = label.title = `Align column ${value}`;
        const glyph = document.createElement('span');
        glyph.innerHTML = icon(alignmentGlyphs[value]);
        label.append(input, glyph);
        alignment.append(label);
        alignments.set(value, input);
        buttons.set(`Align column ${value}`, input);
        input.addEventListener('change', () => {
            if (!input.checked || !owned()) return;
            const s = selected!;
            commit(s, alignTableColumn(s.model, s.column, value), s.row, s.column, `Align column ${value}`);
        });
    }
    columnGroup.append(alignment);
    add(columnGroup, 'Reset column widths', 'refresh');
    // `pending` is set by an edit and consumed by the next `attach`. `epoch` is the document epoch
    // of the last `attach`.
    let selected: Selection | null = null,
        pending: Restore | null = null,
        gesture: Gesture | null = null,
        epoch = -1;
    let cellEditor: CellEditor | null = null;
    let observedCell: HTMLTableCellElement | null = null;
    let frame = 0,
        scrollFrame = 0;
    // A drag ends with a click event on the grip. Clicks before this time do not open the menu.
    let ignoreClickUntil = 0;
    const grids = new WeakMap<HTMLTableElement, HTMLTableCellElement[][]>();
    const tabCells = new WeakMap<HTMLTableElement, HTMLTableCellElement>();
    // True while the selection can be edited. Its table is in the DOM, its model was read from the
    // current source, and the host allows edits. Offsets from a stale model would hit other text.
    const owned = (s = selected) => !!s && s.table.isConnected && s.model.source === host.text() && host.enabled();
    const cells = (table: HTMLTableElement) => grids.get(table) ?? [...table.rows].map((r) => [...r.cells]);
    // The rendered table has one cell for each source cell and no spans. Row and cell indexes of
    // the DOM then address the model directly.
    const compatible = (table: HTMLTableElement, model: TableModel) =>
        table.rows.length === model.rows.length + 1 &&
        [...table.rows].every(
            (r) =>
                r.cells.length === model.header.cells.length &&
                [...r.cells].every((c) => c.colSpan === 1 && c.rowSpan === 1),
        );
    const closePanels = () => {
        for (const panel of Object.values(panels)) if (panel.matches(':popover-open')) panel.hidePopover();
    };
    // Places the grips next to the selected cell, inside the visible part of the pane. Hides them
    // when the cell is out of view or text is selected. With a cell editor open, it sizes the
    // textarea to the cell instead.
    const position = () => {
        if (!owned()) return;
        if (cellEditor) {
            const { cell, input } = cellEditor,
                rect = cell.getBoundingClientRect(),
                box = input.getBoundingClientRect();
            const height = Math.max(72, rect.height - 2 * (box.top - rect.top));
            if (Math.abs(box.height - height) > 0.1) input.style.height = `${height}px`;
            closePanels();
            toolbar.hidden = true;
            return;
        }
        const s = selected!,
            viewport = window.visualViewport,
            paneRect = pane.getBoundingClientRect();
        const left = Math.max(viewport?.offsetLeft ?? 0, paneRect.left) + 8;
        const right = Math.min((viewport?.offsetLeft ?? 0) + (viewport?.width ?? innerWidth), paneRect.right) - 8;
        const top = Math.max(viewport?.offsetTop ?? 0, paneRect.top) + 8;
        const bottom = Math.min((viewport?.offsetTop ?? 0) + (viewport?.height ?? innerHeight), paneRect.bottom) - 8;
        const tableRect = s.table.getBoundingClientRect(),
            wrap = s.table.closest<HTMLElement>('.su-table-edit-scroll')!.getBoundingClientRect();
        const cell = cells(s.table)[s.row][s.column].getBoundingClientRect();
        const rowStyle = getComputedStyle(grips.row),
            columnStyle = getComputedStyle(grips.column);
        const rowWidth = parseFloat(rowStyle.width),
            rowHeight = parseFloat(rowStyle.height);
        const columnWidth = parseFloat(columnStyle.width),
            columnHeight = parseFloat(columnStyle.height);
        const hit = Math.max(rowWidth, rowHeight, columnWidth, columnHeight);
        if (
            right - left < hit ||
            bottom - top < hit ||
            (!gesture && (cell.bottom <= top || cell.top >= bottom || !!window.getSelection()?.toString()))
        ) {
            closePanels();
            toolbar.hidden = true;
            return;
        }
        toolbar.hidden = false;
        const x = Math.max(left, Math.max(tableRect.left, wrap.left) - rowWidth / 2);
        const end = Math.min(right, tableRect.right, wrap.right);
        const rowX = Math.max(x, Math.min(Math.max(cell.left, wrap.left, left) - rowWidth / 2, end - rowWidth));
        const rowY = Math.max(
            top,
            Math.min(
                (gesture?.kind === 'row' ? gesture.clientY : cell.top + cell.height / 2) - rowHeight / 2,
                bottom - rowHeight,
            ),
        );
        const columnY = Math.max(top, Math.min(cell.top - columnHeight / 2, bottom - columnHeight));
        const columnX = Math.max(
            x,
            Math.min(
                (gesture?.kind === 'column' ? gesture.clientX : cell.left + cell.width / 2) - columnWidth / 2,
                end - columnWidth,
            ),
        );
        const y = Math.min(rowY, columnY);
        Object.assign(toolbar.style, {
            left: `${x}px`,
            top: `${y}px`,
            width: `${Math.max(hit, end - x)}px`,
            height: `${Math.max(rowY + rowHeight, columnY + columnHeight) - y}px`,
        });
        Object.assign(grips.row.style, { left: `${rowX - x}px`, top: `${rowY - y}px` });
        Object.assign(grips.column.style, { left: `${columnX - x}px`, top: `${columnY - y}px` });
        grips.column.hidden = gesture?.kind !== 'column' && (cell.right <= wrap.left || cell.left >= wrap.right);
        for (const axis of ['row', 'column'] as const) {
            const panel = panels[axis];
            if (!panel.matches(':popover-open')) continue;
            panel.style.width = `${Math.min(224, right - left)}px`;
            panel.style.maxHeight = `${bottom - top}px`;
            const anchor = grips[axis].querySelector<HTMLElement>('.su-table-grip-face')!.getBoundingClientRect(),
                height = panel.offsetHeight;
            panel.style.left = `${Math.max(left, Math.min(anchor.left, right - panel.offsetWidth))}px`;
            panel.style.top = `${Math.max(top, Math.min(anchor.bottom + 6 + height <= bottom ? anchor.bottom + 6 : anchor.top - height - 6, bottom - height))}px`;
        }
    };
    const openPanel = (axis: 'row' | 'column') => {
        if (!owned()) return;
        const panel = panels[axis];
        if (panel.matches(':popover-open')) {
            panel.hidePopover();
            grips[axis].focus({ preventScroll: true });
            return;
        }
        closePanels();
        panel.showPopover();
        position();
        panel.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus({ preventScroll: true });
    };
    // Selects a cell and reads its table from the current source. Clears the selection when the
    // table cannot be edited. `row` and `column` are clamped to the table.
    const select = (table: HTMLTableElement, row: number, column: number, focus = false) => {
        const model = readTable(host.text(), {
            start: Number(table.dataset.tableStart),
            end: Number(table.dataset.tableEnd),
        });
        if (!model || !compatible(table, model) || !host.enabled()) {
            cancel();
            return;
        }
        closePanels();
        const ordinal = [...article.querySelectorAll('table[data-table-start]')].indexOf(table);
        selected?.table.classList.remove('su-table-active');
        if (selected) cells(selected.table)[selected.row]?.[selected.column]?.classList.remove('su-table-selected');
        selected = {
            table,
            model,
            ordinal,
            row: Math.min(row, model.rows.length),
            column: Math.min(column, model.header.cells.length - 1),
        };
        // A newer selection wins over an edit's delayed render/focus restoration.
        if (pending) pending = { ordinal, row: selected.row, column: selected.column, epoch };
        table.classList.add('su-table-active');
        const grid = cells(table);
        const previousTab = tabCells.get(table) ?? grid[0][0];
        previousTab.tabIndex = -1;
        const cell = grid[selected.row]?.[selected.column];
        if (observedCell !== cell) {
            if (observedCell) layoutObserver.unobserve(observedCell);
            observedCell = cell ?? null;
            if (observedCell) layoutObserver.observe(observedCell);
        }
        cell?.classList.add('su-table-selected');
        if (cell) {
            cell.tabIndex = 0;
            tabCells.set(table, cell);
        }
        rowLabel.textContent = selected.row ? `Row ${selected.row}` : 'Header';
        columnLabel.textContent = `Column ${selected.column + 1}`;
        for (const [name, button] of buttons) {
            button.disabled =
                name === 'Remove column'
                    ? model.header.cells.length <= 1
                    : name === 'Insert row above'
                      ? !selected.row
                      : name === 'Remove row'
                        ? !selected.row
                        : name.startsWith('Drag ')
                          ? false
                          : name === 'Move row up'
                            ? selected.row <= 1
                            : name === 'Move row down'
                              ? !selected.row || selected.row >= model.rows.length
                              : name === 'Move column left'
                                ? selected.column === 0
                                : name === 'Move column right'
                                  ? selected.column >= model.header.cells.length - 1
                                  : name === 'Reset column widths'
                                    ? !model.widths
                                    : false;
        }
        grips.row.dataset.canDrag = String(!!selected.row && model.rows.length > 1);
        grips.column.dataset.canDrag = String(model.header.cells.length > 1);
        for (const [value, input] of alignments) input.checked = columnAlignment(model, selected.column) === value;
        if (focus) {
            cell?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
            cell?.focus({ preventScroll: true });
        }
        toolbar.hidden = false;
        position();
    };
    // Replaces the whole table range, `su-table` comment included, with `text`. `row`, `column`,
    // and `focus` say what to select and focus after the render.
    const commit = (s: Selection, text: string, row = s.row, column = s.column, focus?: string) => {
        if (!owned(s)) {
            cancel();
            return;
        }
        if (text === s.model.source.slice(s.model.start, s.model.end)) return;
        pending = { ordinal: s.ordinal, row, column, epoch, focus };
        host.edit(s.model.start, s.model.end, text, s.table.closest<HTMLElement>('.su-block') ?? s.table);
    };
    // Closes the cell editor and applies its draft as a cell splice. Returns true when the source
    // changed, which starts a render. `focus` returns the focus to the cell.
    function finishCell(focus = false): boolean {
        const ed = cellEditor;
        if (!ed) return false;
        // The draft conflicts with a remote change. It stays open until the reader resolves it.
        if (host.blocked?.()) return false;
        cellEditor = null;
        layoutObserver.unobserve(ed.input);
        const value = ed.input.value,
            s = ed.selection;
        ed.form.remove();
        ed.cell.append(ed.content);
        ed.cell.classList.remove('su-table-cell-editing');
        if (owned(s) && value !== ed.original) {
            const edit = editTableCell(s.model, s.row, s.column, value);
            if (s.model.source.slice(edit.start, edit.end) !== edit.text) {
                pending = { ordinal: s.ordinal, row: s.row, column: s.column, epoch, passive: !focus };
                host.edit(edit.start, edit.end, edit.text, ed.cell);
                return true;
            }
        }
        if (focus && ed.cell.isConnected) ed.cell.focus({ preventScroll: true });
        position();
        return false;
    }
    // Opens a textarea with the source Markdown of the cell. Escape, Ctrl/Cmd+Enter, and a focus
    // move out of the editor apply the draft.
    const openCell = (cell: HTMLTableCellElement, table: HTMLTableElement) => {
        finishCell();
        select(table, (cell.parentElement as HTMLTableRowElement).rowIndex, cell.cellIndex);
        if (!owned()) return;
        host.startCell();
        window.getSelection()?.removeAllRanges();
        closePanels();
        const s = selected!,
            original = (s.row ? s.model.rows[s.row - 1] : s.model.header).cells[s.column].trim();
        const originalHeight = cell.getBoundingClientRect().height;
        const content = document.createDocumentFragment();
        content.append(...cell.childNodes);
        const form = document.createElement('div'),
            label = document.createElement('label'),
            input = document.createElement('textarea');
        const help = document.createElement('span');
        form.className = 'su-table-cell-editor';
        input.id = `su-cell-${crypto.randomUUID()}`;
        label.htmlFor = input.id;
        label.className = 'sr-only';
        label.textContent = 'Edit table cell';
        help.id = `${input.id}-help`;
        help.className = 'sr-only';
        help.textContent =
            'Escape or Ctrl/Cmd+Enter applies the edit. Clicking away also applies it. Enter adds a line break.';
        input.setAttribute('aria-describedby', help.id);
        input.value = original;
        input.cols = 1;
        input.rows = 3;
        input.spellcheck = false;
        form.append(label, help, input);
        cell.append(form);
        cell.classList.add('su-table-cell-editing');
        const inset = input.getBoundingClientRect().top - cell.getBoundingClientRect().top;
        input.style.height = `${Math.max(72, originalHeight - 2 * inset)}px`;
        cellEditor = { selection: s, cell, form, input, content, original };
        layoutObserver.observe(input);
        form.addEventListener(
            'keydown',
            (event) => {
                // An IME composition uses Escape and Enter itself.
                if (event.isComposing || event.keyCode === 229) return;
                if (event.key !== 'Escape' && !(event.key === 'Enter' && (event.metaKey || event.ctrlKey))) return;
                event.preventDefault();
                event.stopPropagation();
                finishCell(true);
            },
            true,
        );
        form.addEventListener('focusout', (event) => {
            if (!form.contains(event.relatedTarget as Node | null)) finishCell();
        });
        position();
        input.focus();
        input.setSelectionRange(input.value.length, input.value.length);
    };
    // Runs the toolbar action with the label `name`. `focus` is the control to focus after the render.
    const action = (name: string, focus = name) => {
        if (!owned()) {
            cancel();
            return;
        }
        const s = selected!;
        if (name === 'Reset column widths') {
            commit(s, resizeTable(s.model, null), s.row, s.column, focus);
            return;
        }
        const axis = name.includes('row') ? 'row' : 'column';
        const index = axis === 'row' ? Math.max(0, s.row - 1) : s.column;
        // The header is not a body row. "Insert row below" on the header inserts before body row 0.
        const kind = name.startsWith('Insert')
            ? /above|before/.test(name) || (axis === 'row' && !s.row)
                ? 'before'
                : 'after'
            : name.startsWith('Remove')
              ? 'remove'
              : 'move';
        const dir = /up|left/.test(name) ? -1 : 1;
        const operation: TableAction = { axis, index, kind, to: index + dir };
        try {
            const next = changeTable(s.model, operation);
            const target =
                kind === 'before' ? index : kind === 'after' ? index + 1 : kind === 'move' ? index + dir : index;
            commit(s, next, axis === 'row' ? target + 1 : s.row, axis === 'column' ? target : s.column, focus);
            host.toast(kind === 'move' ? `Moved ${axis}` : kind === 'remove' ? `Removed ${axis}` : `Inserted ${axis}`);
        } catch (error) {
            host.toast((error as Error).message);
        }
    };
    // Column shares: the saved widths, or the rendered header cell widths as a share of the table.
    const measured = (s: Selection) =>
        s.model.widths ??
        [...s.table.rows[0].cells].map((c) => c.getBoundingClientRect().width / s.table.getBoundingClientRect().width);
    // Moves the border between the selected column and the next one by `dx` pixels. The two
    // columns keep their combined share. Each keeps at least 48 pixels, or a third of the
    // combined share when that is less.
    const resized = (g: Gesture, dx: number) => {
        const widths = [...g.widths],
            i = g.selection.column,
            pair = widths[i] + widths[i + 1];
        const width = g.selection.table.getBoundingClientRect().width;
        const minimum = Math.min(48 / width, pair / 3);
        widths[i] = Math.max(minimum, Math.min(pair - minimum, widths[i] + dx / width));
        widths[i + 1] = pair - widths[i];
        return widths;
    };
    // Starts a drag with the primary button. Pointer capture keeps the events on the handle.
    const begin = (event: PointerEvent, kind: Gesture['kind'], handle: HTMLElement) => {
        if (event.button !== 0 || !owned() || gesture) return;
        if (kind !== 'resize' && handle.dataset.canDrag !== 'true') return;
        event.preventDefault();
        event.stopPropagation();
        handle.focus({ preventScroll: true });
        gesture = {
            selection: selected!,
            kind,
            pointer: event.pointerId,
            x: event.clientX,
            y: event.clientY,
            clientX: event.clientX,
            clientY: event.clientY,
            time: performance.now(),
            widths: measured(selected!),
            initial: selected!.model.widths,
            target: kind === 'row' ? selected!.row - 1 : selected!.column,
            active: false,
            handle,
        };
        handle.setPointerCapture(event.pointerId);
    };
    // Finds the drop position under the pointer among the other rows or columns. Shows the drop
    // line there, clipped to the visible part of the table.
    const updateDrop = (g: Gesture) => {
        const table = g.selection.table,
            from = g.kind === 'row' ? g.selection.row - 1 : g.selection.column;
        const entries = g.kind === 'row' ? [...table.tBodies[0].rows] : [...table.rows[0].cells];
        const remaining = entries.filter((_, i) => i !== from),
            coordinate = g.kind === 'row' ? g.clientY : g.clientX;
        const at = remaining.findIndex((el) => {
            const r = el.getBoundingClientRect();
            return coordinate < (g.kind === 'row' ? r.top + r.height / 2 : r.left + r.width / 2);
        });
        g.target = at < 0 ? remaining.length : at;
        const r = (remaining[g.target] ?? remaining.at(-1) ?? entries[from]).getBoundingClientRect(),
            tableRect = table.getBoundingClientRect();
        const edge =
            g.target < remaining.length ? (g.kind === 'row' ? r.top : r.left) : g.kind === 'row' ? r.bottom : r.right;
        const bounds = table.closest<HTMLElement>('.su-table-edit-scroll')!.getBoundingClientRect();
        const viewport = article.parentElement!.getBoundingClientRect();
        const left = Math.max(0, tableRect.left, bounds.left),
            right = Math.min(window.innerWidth, tableRect.right, bounds.right);
        const top = Math.max(0, tableRect.top, viewport.top),
            bottom = Math.min(window.innerHeight, tableRect.bottom, viewport.bottom);
        Object.assign(
            line.style,
            g.kind === 'row'
                ? {
                      top: `${Math.max(top, Math.min(edge, bottom))}px`,
                      left: `${left}px`,
                      width: `${right - left}px`,
                      height: '2px',
                  }
                : {
                      top: `${top}px`,
                      left: `${Math.max(left, Math.min(edge, right))}px`,
                      width: '2px',
                      height: `${bottom - top}px`,
                  },
        );
        line.hidden = false;
    };
    // Scrolls during a reorder drag while the pointer is within 32 pixels of an edge. A column drag
    // scrolls the table wrapper, and a row drag scrolls the parent of the article. The speed grows
    // to 600 pixels per second.
    const scrollDrag = (time: number) => {
        const g = gesture;
        if (!g?.active || g.kind === 'resize' || !owned(g.selection)) return;
        const scroll =
            g.kind === 'column'
                ? g.selection.table.closest<HTMLElement>('.su-table-edit-scroll')!
                : article.parentElement!;
        const r = scroll.getBoundingClientRect(),
            point = g.kind === 'column' ? g.clientX : g.clientY;
        const low = g.kind === 'column' ? r.left : Math.max(0, r.top),
            high = g.kind === 'column' ? r.right : Math.min(window.innerHeight, r.bottom);
        const pressure =
            point < low + 32
                ? -Math.min(1, (low + 32 - point) / 32)
                : point > high - 32
                  ? Math.min(1, (point - high + 32) / 32)
                  : 0;
        const amount = (pressure * 600 * Math.max(0, Math.min(time - g.time, 50))) / 1000;
        g.time = time;
        if (g.kind === 'column') scroll.scrollLeft += amount;
        else scroll.scrollTop += amount;
        if (pressure) updateDrop(g);
        scrollFrame = requestAnimationFrame(scrollDrag);
    };
    const move = (event: PointerEvent) => {
        const g = gesture;
        if (!g || event.pointerId !== g.pointer) return;
        if (!owned(g.selection)) {
            cancel();
            return;
        }
        if (!g.active && Math.hypot(event.clientX - g.x, event.clientY - g.y) < 4) return;
        if (!g.active) closePanels();
        if (!g.active && g.kind !== 'resize') scrollFrame = requestAnimationFrame(scrollDrag);
        g.active = true;
        g.clientX = event.clientX;
        g.clientY = event.clientY;
        g.handle.classList.add('su-table-dragging');
        if (g.kind === 'resize') {
            const widths = resized(g, event.clientX - g.x);
            setTableWidths(g.selection.table, widths);
            g.handle.setAttribute('aria-valuenow', String(Math.round(widths[g.selection.column] * 100)));
            return;
        }
        updateDrop(g);
    };
    // Ends a drag and commits its result: the new widths, or the move to the drop position.
    const finish = (event: PointerEvent) => {
        const g = gesture;
        if (!g || event.pointerId !== g.pointer) return;
        cancelAnimationFrame(scrollFrame);
        gesture = null;
        line.hidden = true;
        g.handle.classList.remove('su-table-dragging');
        if (g.handle.hasPointerCapture(event.pointerId)) g.handle.releasePointerCapture(event.pointerId);
        if (!g.active || !owned(g.selection)) return;
        ignoreClickUntil = performance.now() + 200;
        try {
            if (g.kind === 'resize')
                commit(g.selection, resizeTable(g.selection.model, resized(g, event.clientX - g.x)));
            else {
                const s = g.selection,
                    index = g.kind === 'row' ? s.row - 1 : s.column;
                if (index !== g.target)
                    commit(
                        s,
                        changeTable(s.model, { axis: g.kind, index, kind: 'move', to: g.target }),
                        g.kind === 'row' ? g.target + 1 : s.row,
                        g.kind === 'column' ? g.target : s.column,
                    );
            }
        } catch (error) {
            if (g.kind === 'resize') setTableWidths(g.selection.table, g.initial);
            host.toast((error as Error).message);
        }
    };
    // Applies an open cell draft, aborts a drag (a resize returns to the saved widths), and clears
    // the selection.
    function cancel() {
        finishCell();
        closePanels();
        if (observedCell) {
            layoutObserver.unobserve(observedCell);
            observedCell = null;
        }
        cancelAnimationFrame(scrollFrame);
        const g = gesture;
        gesture = null;
        line.hidden = true;
        if (g) {
            if (g.kind === 'resize' && g.selection.table.isConnected) setTableWidths(g.selection.table, g.initial);
            g.handle.classList.remove('su-table-dragging');
            if (g.handle.hasPointerCapture(g.pointer)) g.handle.releasePointerCapture(g.pointer);
        }
        selected?.table.classList.remove('su-table-active');
        if (selected) cells(selected.table)[selected.row]?.[selected.column]?.classList.remove('su-table-selected');
        selected = null;
        toolbar.hidden = true;
    }
    toolbar.addEventListener('click', (event) => {
        const button = (event.target as HTMLElement).closest('button');
        if (button && !button.disabled && !button.classList.contains('su-table-grip'))
            action(button.getAttribute('aria-label')!);
    });
    for (const axis of ['row', 'column'] as const) {
        const button = grips[axis];
        button.title = `${axis === 'row' ? 'Row' : 'Column'} actions. Drag to reorder, or use ${axis === 'row' ? 'up/down' : 'left/right'} arrow keys.`;
        button.setAttribute('aria-description', button.title);
        button.addEventListener('click', (event) => {
            event.preventDefault();
            if (performance.now() >= ignoreClickUntil) openPanel(axis);
        });
        button.addEventListener('pointerdown', (e) => begin(e, axis, button));
        button.addEventListener('pointermove', move);
        button.addEventListener('pointerup', finish);
        button.addEventListener('pointercancel', cancel);
        button.addEventListener('lostpointercapture', () => {
            if (gesture) cancel();
        });
        button.addEventListener('keydown', (event) => {
            const backwards = axis === 'row' ? 'ArrowUp' : 'ArrowLeft',
                forwards = axis === 'row' ? 'ArrowDown' : 'ArrowRight';
            if (![backwards, forwards].includes(event.key)) return;
            event.preventDefault();
            event.stopPropagation();
            const name = `Move ${axis} ${axis === 'row' ? (event.key === backwards ? 'up' : 'down') : event.key === backwards ? 'left' : 'right'}`;
            if (!buttons.get(name)!.disabled) action(name, `Drag ${axis} to reorder`);
        });
    }
    article.addEventListener('click', (event) => {
        if (gesture || (event.target as HTMLElement).closest('a, input, textarea, button')) return;
        const cell = (event.target as HTMLElement).closest<HTMLTableCellElement>('th, td'),
            table = cell?.closest<HTMLTableElement>('table[data-table-start]');
        if (cell && table && !window.getSelection()?.toString())
            select(table, (cell.parentElement as HTMLTableRowElement).rowIndex, cell.cellIndex);
        else cancel();
    });
    article.addEventListener('dblclick', (event) => {
        if (
            (event.target as HTMLElement).closest(
                'a, input, textarea, button, .su-table-cell-editor, .su-diagram, .su-mermaid',
            )
        )
            return;
        const cell = (event.target as HTMLElement).closest<HTMLTableCellElement>('th, td'),
            table = cell?.closest<HTMLTableElement>('table[data-table-start]');
        if (!cell || !table || !host.enabled()) return;
        event.preventDefault();
        event.stopImmediatePropagation();
        openCell(cell, table);
    });
    article.addEventListener('focusin', (event) => {
        const cell = (event.target as HTMLElement).closest<HTMLTableCellElement>('th, td'),
            table = cell?.closest<HTMLTableElement>('table[data-table-start]');
        if (cell && table && !gesture)
            select(table, (cell.parentElement as HTMLTableRowElement).rowIndex, cell.cellIndex);
    });
    article.addEventListener('keydown', (event) => {
        if ((event.target as HTMLElement).closest('input, textarea, button') || !owned()) return;
        if (event.key === 'Enter') {
            const cell = (event.target as HTMLElement).closest<HTMLTableCellElement>('th, td');
            if (cell) {
                event.preventDefault();
                event.stopPropagation();
                openCell(cell, selected!.table);
            }
            return;
        }
        const direction = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] }[event.key];
        if (!direction || event.ctrlKey || event.metaKey) return;
        event.preventDefault();
        event.stopPropagation();
        select(
            selected!.table,
            Math.max(0, selected!.row + direction[0]),
            Math.max(0, selected!.column + direction[1]),
            true,
        );
    });
    document.addEventListener(
        'pointerdown',
        (event) => {
            if (!toolbar.contains(event.target as Node) && !article.contains(event.target as Node)) cancel();
        },
        true,
    );
    document.addEventListener(
        'keydown',
        (event) => {
            if (event.key !== 'Escape' || (!gesture && toolbar.hidden)) return;
            event.preventDefault();
            event.stopPropagation();
            const axis = (['row', 'column'] as const).find((name) => panels[name].matches(':popover-open'));
            if (axis && !gesture) {
                closePanels();
                grips[axis].focus({ preventScroll: true });
            } else cancel();
        },
        true,
    );
    const schedulePosition = () => {
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(position);
    };
    const layoutObserver = new ResizeObserver(schedulePosition);
    layoutObserver.observe(pane);
    layoutObserver.observe(article);
    document.addEventListener('scroll', schedulePosition, true);
    window.addEventListener('resize', schedulePosition);
    document.addEventListener('selectionchange', schedulePosition);
    window.visualViewport?.addEventListener('resize', schedulePosition);
    window.visualViewport?.addEventListener('scroll', schedulePosition);

    return {
        cancel,
        flush: () => finishCell(),
        editing: () => !!cellEditor,
        draft: () => {
            if (!cellEditor) return null;
            const ed = cellEditor,
                change = editTableCell(ed.selection.model, ed.selection.row, ed.selection.column, ed.input.value);
            return { start: change.start, end: change.end, text: change.text };
        },
        discardDraft: () => {
            const ed = cellEditor;
            if (!ed) return;
            cellEditor = null;
            layoutObserver.unobserve(ed.input);
            ed.form.remove();
            ed.cell.append(ed.content);
            ed.cell.classList.remove('su-table-cell-editing');
        },
        attach(source: string, nextEpoch: number) {
            // A render of the same source during a reorder drag keeps the drag. Every other render
            // clears the gesture and the selection, and `restore` selects the cell again.
            const moving =
                gesture && gesture.kind !== 'resize' && epoch === nextEpoch && gesture.selection.model.source === source
                    ? gesture
                    : null;
            const activeFocus = [...buttons].find(([, button]) => button === document.activeElement)?.[0];
            const restore =
                pending ??
                (selected && selected.model.source === source && epoch === nextEpoch
                    ? {
                          ordinal: selected.ordinal,
                          row: selected.row,
                          column: selected.column,
                          epoch,
                          focus: activeFocus,
                      }
                    : null);
            pending = null;
            if (!moving) cancel();
            epoch = nextEpoch;
            for (const table of article.querySelectorAll<HTMLTableElement>('table[data-table-start]')) {
                const model = readTable(source, {
                    start: Number(table.dataset.tableStart),
                    end: Number(table.dataset.tableEnd),
                });
                // A table without a matching source model stays read-only.
                if (!model || !compatible(table, model)) continue;
                let wrap = table.parentElement!;
                if (!wrap.matches('.mdp-table-wrap')) {
                    wrap = document.createElement('div');
                    wrap.className = 'mdp-table-wrap';
                    table.before(wrap);
                    wrap.append(table);
                }
                wrap.classList.add('su-table-edit-scroll');
                wrap.style.setProperty('--table-columns', String(table.rows[0].cells.length));
                wrap.tabIndex = -1;
                const grid = cells(table);
                grids.set(table, grid);
                // Roving tabindex: one cell of each table is in the tab order, and arrow keys move it.
                for (const row of grid) for (const cell of row) cell.tabIndex = -1;
                grid[0][0].tabIndex = 0;
                tabCells.set(table, grid[0][0]);
                for (const [column, th] of [...table.rows[0].cells].entries()) {
                    // A handle resizes its column against the next one, so the last column has none.
                    if (column === grid[0].length - 1) continue;
                    th.classList.add('su-table-header-cell');
                    const handle = document.createElement('button');
                    handle.type = 'button';
                    handle.className = 'su-table-resize';
                    handle.setAttribute('role', 'separator');
                    handle.setAttribute('aria-orientation', 'vertical');
                    handle.setAttribute('aria-label', `Resize column ${column + 1}`);
                    handle.setAttribute('aria-valuemin', '0');
                    handle.setAttribute('aria-valuemax', '100');
                    handle.setAttribute(
                        'aria-valuenow',
                        String(Math.round((model.widths?.[column] ?? 1 / grid[0].length) * 100)),
                    );
                    handle.title = 'Drag to resize. Arrow keys adjust width.';
                    th.append(handle);
                    handle.addEventListener('pointerdown', (event) => {
                        select(table, 0, column);
                        begin(event, 'resize', handle);
                    });
                    handle.addEventListener('pointermove', move);
                    handle.addEventListener('pointerup', finish);
                    handle.addEventListener('pointercancel', cancel);
                    handle.addEventListener('lostpointercapture', () => {
                        if (gesture) cancel();
                    });
                    handle.addEventListener('keydown', (event) => {
                        // Keyboard resize: 10 pixels for each key press, 30 with Shift.
                        if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) return;
                        event.preventDefault();
                        event.stopPropagation();
                        select(table, 0, column);
                        if (!owned()) return;
                        const s = selected!,
                            widths = measured(s);
                        const g = { selection: s, widths } as Gesture;
                        commit(
                            s,
                            resizeTable(
                                s.model,
                                resized(g, (event.key === 'ArrowLeft' ? -1 : 1) * (event.shiftKey ? 30 : 10)),
                            ),
                            0,
                            column,
                            `Resize column ${column + 1}`,
                        );
                    });
                }
            }
            if (moving) {
                const table = [...article.querySelectorAll<HTMLTableElement>('table[data-table-start]')][
                    moving.selection.ordinal
                ];
                if (table) {
                    select(table, moving.selection.row, moving.selection.column);
                    if (selected) {
                        moving.selection = selected;
                        if (moving.active) updateDrop(moving);
                        return;
                    }
                }
                cancel();
            } else if (restore && restore.epoch === nextEpoch) {
                const table = [...article.querySelectorAll<HTMLTableElement>('table[data-table-start]')][
                    restore.ordinal
                ];
                if (table) {
                    select(table, restore.row, restore.column, !restore.focus && !restore.passive);
                    if (restore.focus && selected) {
                        cells(table)[selected.row]?.[selected.column]?.scrollIntoView({
                            block: 'nearest',
                            inline: 'nearest',
                        });
                        position();
                    }
                    const button =
                        restore.focus &&
                        (buttons.get(restore.focus) ??
                            [...table.querySelectorAll<HTMLElement>('.su-table-resize')].find(
                                (b) => b.getAttribute('aria-label') === restore.focus,
                            ));
                    if (button && !(button as HTMLButtonElement).disabled) {
                        const axis = (['row', 'column'] as const).find((name) => panels[name].contains(button));
                        if (axis) {
                            cells(table)[restore.row]?.[restore.column]?.scrollIntoView({
                                block: 'nearest',
                                inline: 'nearest',
                            });
                            position();
                            openPanel(axis);
                        }
                        button.focus({ preventScroll: true });
                    } else if (restore.focus && selected)
                        cells(table)[selected.row]?.[selected.column]?.focus({ preventScroll: true });
                }
            }
        },
    };
}
