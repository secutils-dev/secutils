// Native dialog shared by source review and GitHub conflict resolution. Source is always text.
import { diffWords, sourceLines, textEdits } from './diff.ts';
import type { MergePart } from '../github/merge.ts';
import { createMergeEditor } from './editor.js';
import { hideTooltip } from '../ui/feedback.ts';
import { icon } from '../ui/icons.ts';
import { dialogCloseButton } from '../ui/ui.ts';

const tag = <K extends keyof HTMLElementTagNameMap>(name: K, value = '', className = '') => {
    const node = document.createElement(name);
    node.textContent = value;
    node.className = className;
    return node;
};
const button = (label: string, run: () => void, primary = false) => {
    const element = tag('button', label, `btn${primary ? ' btn-primary' : ''}`);
    element.type = 'button';
    element.addEventListener('click', run);
    return element;
};
function navigation(label: string, run: () => void, previous = false) {
    const element = button('', run);
    element.className = previous ? 'icon-btn mars-review-previous' : 'icon-btn';
    element.setAttribute('aria-label', label);
    element.dataset.tooltip = label;
    element.innerHTML = icon('chevronDown');
    return element;
}
function workspace(title: string) {
    if (document.getElementById('sourceReviewDialog')) return;
    const dialog = tag('dialog', '', 'su-dialog mars-review-workspace');
    dialog.id = 'sourceReviewDialog';
    const heading = tag('h2', title);
    heading.id = 'sourceReviewTitle';
    dialog.setAttribute('aria-labelledby', heading.id);
    const header = tag('header', '', 'su-dialog-header');
    heading.tabIndex = -1;
    heading.autofocus = true;
    const close = dialogCloseButton(() => dialog.close());
    const status = tag('p', '', 'mars-review-status');
    status.setAttribute('role', 'status');
    const controls = tag('div', '', 'mars-review-controls');
    controls.append(status);
    const footer = tag('footer', '', 'mars-modal-foot');
    header.append(heading, close);
    dialog.append(header);
    dialog.addEventListener(
        'close',
        () => {
            hideTooltip();
            dialog.remove();
        },
        { once: true },
    );
    return { dialog, controls, status, footer };
}

function sourceCell(value: string | undefined, line: number, opposite?: string) {
    const cell = tag('div', '', 'mars-review-cell');
    const number = tag('span', value === undefined ? '' : String(line), 'mars-review-line-number');
    number.setAttribute('aria-hidden', 'true');
    const code = tag('code');
    const plain = (value ?? '').replace(/\r\n?$|\n$/, '');
    const ranges =
        opposite === undefined || plain.length > 2048
            ? []
            : (diffWords(opposite.replace(/\r\n?$|\n$/, ''), plain) ?? []);
    let at = 0;
    for (const [start, end] of ranges) {
        code.append(plain.slice(at, start), tag('mark', plain.slice(start, end)));
        at = end;
    }
    code.append(plain.slice(at));
    cell.append(number, code);
    return cell;
}

/** Open an aligned before/after source comparison. The existing preview marks remain independent. */
export function openSourceReview(
    title: string,
    beforeLabel: string,
    before: string,
    after: string,
    afterLabel = 'Current',
) {
    const view = workspace('Compare changes');
    if (!view) return;
    const { dialog, controls, status } = view;
    const headings = tag('div', '', 'mars-review-column-headings');
    headings.append(tag('strong', beforeLabel), tag('strong', afterLabel));
    const body = tag('div', '', 'mars-review-diff');
    body.tabIndex = 0;
    body.setAttribute('role', 'region');
    body.setAttribute('aria-label', 'Aligned before and current source');
    const a = sourceLines(before),
        b = sourceLines(after),
        edits = textEdits(a, b);
    const targets: HTMLElement[] = [];
    const row = (i: number, j: number, changed = false) => {
        const line = tag('div', '', `mars-review-row${changed ? ' is-changed' : ''}`);
        line.append(
            sourceCell(a[i], i + 1, changed ? b[j] : undefined),
            sourceCell(b[j], j + 1, changed ? a[i] : undefined),
        );
        return line;
    };
    let i = 0,
        j = 0;
    const equal = (end: number) => {
        // Keep large unchanged ranges collapsible. Build their rows only when opened.
        const count = end - i;
        if (count > 14) {
            for (let n = 0; n < 4; n++) body.append(row(i++, j++));
            const startA = i,
                startB = j,
                skipped = count - 8;
            const expander = tag('details', '', 'mars-review-gap');
            expander.append(tag('summary', `${skipped} unchanged lines`));
            expander.addEventListener('toggle', () => {
                if (expander.open && expander.childElementCount === 1)
                    for (let n = 0; n < skipped; n++) expander.append(row(startA + n, startB + n));
            });
            body.append(expander);
            i += skipped;
            j += skipped;
        }
        while (i < end) body.append(row(i++, j++));
    };
    for (const edit of edits) {
        equal(edit.start);
        const group = tag('div', '', 'mars-review-hunk');
        const aCount = edit.end - i,
            bCount = edit.replacement.length;
        for (let k = 0; k < Math.max(aCount, bCount); k++)
            group.append(row(k < aCount ? i + k : -1, k < bCount ? j + k : -1, true));
        targets.push(group);
        body.append(group);
        i += aCount;
        j += bCount;
    }
    equal(a.length);
    let current = -1;
    const go = (step: number) => {
        if (!targets.length) return;
        targets[current]?.classList.remove('is-current');
        current =
            current < 0 ? (step < 0 ? targets.length - 1 : 0) : (current + step + targets.length) % targets.length;
        targets[current].classList.add('is-current');
        targets[current].scrollIntoView({ block: 'center' });
        status.textContent = `Change ${current + 1} of ${targets.length}`;
    };
    const previous = navigation('Previous change', () => go(-1), true),
        next = navigation('Next change', () => go(1));
    previous.disabled = next.disabled = !targets.length;
    status.textContent = `${targets.length} change${targets.length === 1 ? '' : 's'}`;
    const toolbar = tag('div', '', 'mars-review-toolbar');
    controls.append(previous, next);
    toolbar.append(tag('p', title, 'mars-review-filename'), controls);
    dialog.append(toolbar, headings, body);
    document.body.append(dialog);
    dialog.showModal();
    go(1);
}

/** Show the full working copies beside an editable merge result. Conflict choices act on one hunk. */
export function openMergeReview(
    title: string,
    parts: MergePart[],
    versions: { local: string; remote: string },
    apply: (text: string) => Promise<void>,
) {
    const view = workspace('Resolve conflicts');
    if (!view) return;
    const { dialog, controls, status, footer } = view;
    const conflicts = parts.flatMap((part) => ('text' in part ? [] : [{ ...part, resolved: false }]));
    if (!conflicts.length) return;
    dialog.classList.add('mars-merge-workspace');
    const normalize = (text: string) => text.replace(/\r\n?/g, '\n');
    const source = () => {
        let text = '';
        let exact = '';
        const ranges: { from: number; to: number; active?: boolean }[] = [];
        for (const part of parts) {
            if ('text' in part) {
                exact += part.text;
                text += normalize(part.text);
            } else {
                const from = text.length;
                exact += part.base;
                text += normalize(part.base);
                ranges.push({ from, to: text.length });
            }
        }
        return { text, exact, ranges };
    };
    const readOnlySource = (which: 'local' | 'remote') => {
        const text = normalize(versions[which]);
        let cursor = 0;
        const ranges = conflicts.map((conflict) => {
            const value = normalize(conflict[which]);
            const from = Math.max(cursor, text.indexOf(value, cursor));
            cursor = Math.min(text.length, from + value.length);
            return { from, to: cursor, active: false };
        });
        return { text, ranges };
    };
    const local = readOnlySource('local'),
        remote = readOnlySource('remote'),
        result = source();
    const exactLines = sourceLines(result.exact),
        variantLines = sourceLines(result.text);
    const exactResult = (text: string) => {
        const next = sourceLines(text),
            edits = textEdits(variantLines, next);
        const newline = result.exact.match(/\r\n|\r|\n/)?.[0] ?? '\n';
        const out: string[] = [];
        let from = 0;
        for (const edit of edits) {
            out.push(exactLines.slice(from, edit.start).join(''));
            out.push(
                edit.replacement
                    .map((line, i) =>
                        line.replace(/\n$/, exactLines[edit.start + i]?.match(/\r\n$|\r$|\n$/)?.[0] ?? newline),
                    )
                    .join(''),
            );
            from = edit.end;
        }
        return out.join('') + exactLines.slice(from).join('');
    };
    const fallback = tag('textarea', '', 'mars-merge-fallback');
    fallback.setAttribute('aria-label', 'Result');
    fallback.value = result.text;
    let editor: Awaited<ReturnType<typeof createMergeEditor>> | undefined;
    let left: typeof editor, right: typeof editor;
    let current = 0;
    let saving = false;
    let editedRange: number | undefined;
    const panes = tag('div', '', 'mars-review-merge');
    const pane = (label: string, command: HTMLButtonElement, initial?: string) => {
        const element = tag('section', '', 'mars-review-pane');
        const top = tag('header', '', 'mars-merge-pane-head');
        top.append(tag('h3', label), command);
        command.classList.add('mars-merge-accept');
        const mount = tag('div', '', 'mars-merge-editor');
        if (initial !== undefined) {
            const pre = tag('pre', initial);
            pre.tabIndex = 0;
            mount.append(pre);
        }
        element.append(top, mount);
        panes.append(element);
        return mount;
    };
    const mine = pane(
        'Local',
        button('Use local →', () => choose(conflicts[current].local)),
        local.text,
    );
    const middle = pane(
        'Result',
        button('Mark resolved', () => {
            conflicts[current].resolved = true;
            update();
        }),
    );
    middle.append(fallback);
    const theirs = pane(
        'GitHub',
        button('← Use GitHub', () => choose(conflicts[current].remote)),
        remote.text,
    );
    const getText = () => editor?.getText() ?? fallback.value;
    let snapshot = result.text;
    // The editor owns all text. Keep hunk ranges anchored as the reader edits the result.
    const track = (text: string) => {
        if (text === snapshot) return;
        let from = 0,
            tail = 0;
        while (from < snapshot.length && from < text.length && snapshot[from] === text[from]) from++;
        while (
            tail < snapshot.length - from &&
            tail < text.length - from &&
            snapshot.at(-1 - tail) === text.at(-1 - tail)
        )
            tail++;
        const to = snapshot.length - tail,
            delta = text.length - snapshot.length,
            after = to + delta;
        for (const [index, range] of result.ranges.entries()) {
            if (range.to < from || (range.to === from && index !== current)) continue;
            if (range.from > to || (range.from === to && index !== current)) {
                range.from += delta;
                range.to += delta;
                continue;
            }
            range.from = Math.min(range.from, from);
            range.to = range.to > to ? range.to + delta : after;
            conflicts[index].resolved = false;
            editedRange ??= index;
        }
        snapshot = text;
        queueMicrotask(() => {
            if (editedRange !== undefined) current = editedRange;
            editedRange = undefined;
            if (dialog.open) update();
        });
    };
    fallback.addEventListener('input', () => track(fallback.value));
    const error = tag('p', '', 'sync-error');
    error.setAttribute('role', 'alert');
    error.hidden = true;
    const update = () => {
        const remaining = conflicts.filter((conflict) => !conflict.resolved).length;
        status.textContent = `Conflict ${current + 1} of ${conflicts.length} · ${remaining} left`;
        save.disabled = saving || remaining > 0;
        const highlights = (ranges: typeof result.ranges, hideResolved = false) =>
            ranges.flatMap((range, i) =>
                hideResolved && conflicts[i].resolved ? [] : [{ ...range, active: i === current }],
            );
        left?.highlight(highlights(local.ranges));
        right?.highlight(highlights(remote.ranges));
        editor?.highlight(highlights(result.ranges, true));
    };
    const show = () => {
        left?.reveal(local.ranges[current].from);
        right?.reveal(remote.ranges[current].from);
        editor?.reveal(result.ranges[current].from);
        update();
    };
    const advance = (step: number) => {
        current = (current + step + conflicts.length) % conflicts.length;
        show();
    };
    const choose = (text: string) => {
        const { from, to } = result.ranges[current];
        const insert = normalize(text);
        if (editor) editor.replace(from, to, insert);
        else {
            fallback.setRangeText(insert, from, to);
            track(fallback.value);
        }
        conflicts[current].resolved = true;
        editedRange = undefined;
        const next = conflicts.findIndex((part, i) => i > current && !part.resolved);
        if (next >= 0) current = next;
        show();
    };
    const previous = navigation('Previous conflict', () => advance(-1), true);
    const next = navigation('Next conflict', () => advance(1));
    previous.disabled = next.disabled = conflicts.length < 2;
    controls.append(previous, next);
    const save = button(
        'Apply changes',
        () => {
            saving = true;
            update();
            error.hidden = true;
            void apply(exactResult(getText())).then(
                () => dialog.close(),
                (reason: unknown) => {
                    saving = false;
                    error.textContent = reason instanceof Error ? reason.message : 'Could not apply this resolution.';
                    error.hidden = false;
                    update();
                },
            );
        },
        true,
    );
    footer.append(save);
    const details = tag('div', '', 'mars-merge-description');
    details.append(
        tag('strong', title),
        tag('span', 'Independent edits are already included. Pick a side or edit the result.'),
    );
    const toolbar = tag('div', '', 'mars-review-toolbar');
    toolbar.append(details, controls);
    dialog.append(toolbar, panes, error, footer);
    document.body.append(dialog);
    dialog.showModal();
    show();
    dialog.addEventListener(
        'close',
        () => {
            editor?.destroy();
            left?.destroy();
            right?.destroy();
        },
        { once: true },
    );
    const mount = async (target: HTMLElement, text: string, label: string, readOnly: boolean) => {
        const host = tag('div', '', 'mars-merge-editor-host');
        target.append(host);
        try {
            const instance = await createMergeEditor(host, text, {
                label,
                readOnly,
                onChange: readOnly ? undefined : track,
            });
            if (!dialog.open) {
                instance.destroy();
                return;
            }
            for (const child of [...target.children]) if (child !== host) child.remove();
            return instance;
        } catch {
            host.remove();
        }
    };
    void mount(mine, local.text, 'Local', true).then((instance) => {
        left = instance;
        if (dialog.open) show();
    });
    void mount(middle, snapshot, 'Result', false).then((instance) => {
        editor = instance;
        if (editor && editor.getText() !== snapshot) editor.replace(0, editor.getText().length, snapshot);
        if (dialog.open) show();
    });
    void mount(theirs, remote.text, 'GitHub', true).then((instance) => {
        right = instance;
        if (dialog.open) show();
    });
}
