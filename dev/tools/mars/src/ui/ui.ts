// Small EUI-like toolset: icon hydration, a context menu, a modal, clipboard, and downloads.
//
// The page has one menu element (`#marsMenu`) and one modal element (`#marsModal`), so one of
// each is open at a time. Trust: `modal({ html })` inserts markup as it is. The caller escapes
// every interpolated value with `esc()`. `modal({ body })` and menu labels are escaped here.
import { icon, type IconName } from './icons.ts';
export { hydrateIcons } from '../../../shared/icons.js';
import { hydrateIcons } from '../../../shared/icons.js';

/** The shell element with this id. The id must exist: the result is not checked for null. */
export const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
/**
 * Escapes `&`, `<`, `>`, and `"` for HTML text and for a double-quoted attribute value.
 * It does not escape `'`, so it is not safe inside a single-quoted attribute.
 */
export const esc = (s: string) =>
    s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** One action of a context menu. `sub` is a second line under the label. */
export interface MenuItem {
    label: string;
    icon?: IconName;
    sub?: string;
    danger?: boolean;
    disabled?: boolean;
    run?: () => void;
}
/** A menu row: an action, a separator line, or a hint paragraph that is not interactive. */
export type MenuEntry = MenuItem | 'separator' | { hint: string };

const menuEl = () => $('marsMenu');
let menuCleanup: (() => void) | null = null;

/** Closes the context menu and removes its document listeners. Safe when no menu is open. */
export function closeMenu() {
    menuCleanup?.();
    menuCleanup = null;
}

/**
 * Opens the shared context menu below `anchor` (an element) or at a pointer position.
 * An open menu closes first. The menu stays inside the viewport and opens above the anchor when
 * there is no room below. It closes on an outside press, on Escape (focus returns to an anchor
 * element), on window blur, and after an action. Arrow keys move between enabled actions.
 */
export function openMenu(anchor: Element | { x: number; y: number }, entries: MenuEntry[], label = 'Actions') {
    closeMenu();
    const menu = menuEl();
    menu.setAttribute('aria-label', label);
    menu.replaceChildren(
        ...entries.map((entry) => {
            if (entry === 'separator') return Object.assign(document.createElement('hr'), { className: 'menu-sep' });
            if ('hint' in entry)
                return Object.assign(document.createElement('p'), {
                    className: 'mars-menu-hint',
                    textContent: entry.hint,
                });
            const b = document.createElement('button');
            b.type = 'button';
            b.className = `export-item${entry.danger ? ' danger' : ''}`;
            b.setAttribute('role', 'menuitem');
            b.disabled = !!entry.disabled;
            b.innerHTML = `${entry.icon ? `<span data-icon="${entry.icon}"></span>` : ''}<span>${esc(entry.label)}${entry.sub ? `<span class="item-sub">${esc(entry.sub)}</span>` : ''}</span>`;
            b.addEventListener('click', () => {
                closeMenu();
                entry.run?.();
            });
            return b;
        }),
    );
    hydrateIcons(menu);
    menu.hidden = false;
    const { width, height } = menu.getBoundingClientRect();
    const r =
        anchor instanceof Element
            ? anchor.getBoundingClientRect()
            : { left: anchor.x, right: anchor.x, top: anchor.y, bottom: anchor.y };
    const left = anchor instanceof Element ? r.right - width : r.left;
    menu.style.left = `${Math.round(Math.min(Math.max(8, left), innerWidth - width - 8))}px`;
    menu.style.top = `${Math.round(r.bottom + 4 + height > innerHeight - 8 ? Math.max(8, r.top - height - 4) : r.bottom + 4)}px`;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    items[0]?.focus();
    const onDown = (e: PointerEvent) => {
        if (!menu.contains(e.target as Node)) closeMenu();
    };
    const onKey = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
            e.stopPropagation();
            closeMenu();
            if (anchor instanceof HTMLElement) anchor.focus();
        } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            const i = items.indexOf(document.activeElement as HTMLButtonElement);
            items[(i + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
        }
    };
    // Listen after this task, so the press that opened the menu does not close it.
    setTimeout(() => document.addEventListener('pointerdown', onDown, true));
    document.addEventListener('keydown', onKey, true);
    window.addEventListener('blur', closeMenu, { once: true });
    menuCleanup = () => {
        menu.hidden = true;
        document.removeEventListener('pointerdown', onDown, true);
        document.removeEventListener('keydown', onKey, true);
    };
}

/** Visual variants of the shared button style. `primary` wins over `danger`, and `danger` over `empty`. */
export interface ButtonAppearance {
    primary?: boolean;
    danger?: boolean;
    empty?: boolean;
    text?: boolean;
    flush?: 'left' | 'right' | 'both';
}
/** The class string for a button with the shared style. */
export const buttonClass = (button: ButtonAppearance = {}) =>
    `btn${button.primary ? ' btn-primary' : button.danger ? ' btn-danger' : button.empty ? ' btn-empty' : ''}${button.text ? ' btn-text' : ''}${button.flush ? ` btn-flush-${button.flush}` : ''}`;
/** One footer button of a modal. `value` is the result when the reader clicks it. */
export interface ModalButton {
    label: string;
    value: string;
    primary?: boolean;
    danger?: boolean;
    empty?: boolean;
}

/**
 * Resolves with the clicked button's value, or '' when dismissed. A `field` adds a text input
 * whose value is returned for the primary button as `{ value, text }`.
 *
 * `body` is plain text. `html` is trusted markup: escape each interpolated value with `esc()`.
 * An aborted `signal` closes the modal and resolves with ''. `text` is the trimmed field value.
 * A call while the modal is open overwrites its content, so callers open one modal at a time.
 */
export function modal(opts: {
    title: string;
    body?: string;
    html?: string;
    buttons: ModalButton[];
    field?: { value: string; label: string; type?: 'text' | 'password' };
    signal?: AbortSignal;
}): Promise<{ value: string; text: string }> {
    if (opts.signal?.aborted) return Promise.resolve({ value: '', text: '' });
    const dlg = $<HTMLDialogElement>('marsModal');
    $('marsModalTitle').textContent = opts.title;
    const body = $('marsModalBody');
    body.innerHTML = opts.html ?? (opts.body ? `<p>${esc(opts.body)}</p>` : '');
    let input: HTMLInputElement | null = null;
    if (opts.field) {
        const label = document.createElement('label');
        label.className = 'field-label';
        label.textContent = opts.field.label;
        label.htmlFor = 'marsModalInput';
        input = Object.assign(document.createElement('input'), {
            id: label.htmlFor,
            className: 'field',
            type: opts.field.type ?? 'text',
            value: opts.field.value,
            autocomplete: 'off',
            spellcheck: false,
        });
        // Implicit submission would pick the first (Cancel) button.
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.isComposing) {
                e.preventDefault();
                dlg.close(opts.buttons.find((b) => b.primary)?.value ?? '');
            }
        });
        body.append(label, input);
    }
    $('marsModalFoot').replaceChildren(
        ...opts.buttons.map((b, index) => {
            const el = Object.assign(document.createElement('button'), {
                type: 'submit',
                value: b.value,
                textContent: b.label,
            });
            // A button whose value means dismiss renders as a text button, unless it has an emphasis.
            const empty = !b.primary && !b.danger && (b.empty ?? ['', 'close', 'back', 'cancel'].includes(b.value));
            el.className = buttonClass({
                ...b,
                empty,
                text: empty,
                flush: empty && index === opts.buttons.length - 1 ? 'right' : undefined,
            });
            return el;
        }),
    );
    dlg.returnValue = '';
    dlg.showModal();
    if (input) {
        input.focus();
        input.select();
    } else
        (
            dlg.querySelector<HTMLButtonElement>('.btn-primary') ??
            dlg.querySelector<HTMLButtonElement>('footer button')
        )?.focus();
    return new Promise((resolve) => {
        const cancel = () => {
            if (dlg.open) dlg.close('');
        };
        opts.signal?.addEventListener('abort', cancel, { once: true });
        dlg.addEventListener(
            'close',
            () => {
                opts.signal?.removeEventListener('abort', cancel);
                resolve({ value: opts.signal?.aborted ? '' : dlg.returnValue, text: input?.value.trim() ?? '' });
            },
            { once: true },
        );
    });
}

/** Asks for confirmation of a destructive action. Returns true only for the danger button. */
export const confirmDanger = async (title: string, body: string, label: string) =>
    (
        await modal({
            title,
            body,
            buttons: [
                { label: 'Cancel', value: '' },
                { label, value: 'ok', danger: true },
            ],
        })
    ).value === 'ok';

/** Asks for one line of text. Returns the trimmed text, or null when cancelled or empty. */
export async function prompt(title: string, label: string, value: string, action = 'Save') {
    const r = await modal({
        title,
        field: { value, label },
        buttons: [
            { label: 'Cancel', value: '' },
            { label: action, value: 'ok', primary: true },
        ],
    });
    return r.value === 'ok' && r.text ? r.text : null;
}

/**
 * Writes text, and optionally an HTML flavor, to the clipboard.
 * Safari keeps the click's user activation only for a clipboard write started synchronously,
 * so the write starts at once with promises that settle after the async work.
 * Call it inside the click handler, before the first `await`.
 * When the `ClipboardItem` write fails but the text resolved, it retries with `writeText`,
 * which copies the plain text only. Rejects when the text promise rejects or the browser
 * refuses both writes.
 */
export async function copyRich(text: Promise<string> | string, html?: Promise<string> | string) {
    const plain = Promise.resolve(text),
        rich = html === undefined ? null : Promise.resolve(html);
    if (typeof ClipboardItem === 'function' && navigator.clipboard?.write) {
        try {
            const items: Record<string, Promise<Blob>> = {
                'text/plain': plain.then((t) => new Blob([t], { type: 'text/plain' })),
            };
            if (rich) items['text/html'] = rich.then((t) => new Blob([t], { type: 'text/html' }));
            await navigator.clipboard.write([new ClipboardItem(items)]);
            return;
        } catch (err) {
            if (
                await plain.then(
                    () => false,
                    () => true,
                )
            )
                throw err;
        }
    }
    await navigator.clipboard.writeText(await plain);
}

/**
 * Starts a browser download of `data` under the file name `name`, with MIME type `type`.
 * The temporary object URL is revoked after 1 second.
 */
export function download(name: string, data: BlobPart, type: string) {
    const url = URL.createObjectURL(new Blob([data], { type }));
    const a = Object.assign(document.createElement('a'), { href: url, download: name });
    a.hidden = true;
    (document.querySelector('dialog:modal') ?? document.body).append(a);
    a.click();
    setTimeout(() => {
        a.remove();
        URL.revokeObjectURL(url);
    }, 1000);
}

/**
 * Makes a document name usable as a file name. The result has no reserved or control
 * characters, no leading or trailing dot or space, and at most 120 characters.
 * An empty result becomes "Untitled".
 */
export const fileSafe = (name: string) =>
    name
        .replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '-')
        .replace(/^[.\s]+|[.\s]+$/g, '')
        .slice(0, 120) || 'Untitled';
