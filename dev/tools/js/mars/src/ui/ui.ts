// Small EUI-like toolset: icon hydration, a context menu, a modal, clipboard, and downloads.
import { icon, type IconName } from './icons.ts';
export { hydrateIcons } from '../../../../shared/icons.js';
import { hydrateIcons } from '../../../../shared/icons.js';

export const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
export const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

export interface MenuItem {
    label: string;
    icon?: IconName;
    sub?: string;
    danger?: boolean;
    disabled?: boolean;
    run?: () => void;
}
export type MenuEntry = MenuItem | 'separator';

const menuEl = () => $('marsMenu');
let menuCleanup: (() => void) | null = null;

export function closeMenu() {
    menuCleanup?.();
    menuCleanup = null;
}

// Opens the shared context menu below `anchor` (an element) or at a pointer position.
export function openMenu(anchor: Element | { x: number; y: number }, entries: MenuEntry[], label = 'Actions') {
    closeMenu();
    const menu = menuEl();
    menu.setAttribute('aria-label', label);
    menu.replaceChildren(...entries.map((entry) => {
        if (entry === 'separator') return Object.assign(document.createElement('hr'), { className: 'menu-sep' });
        const b = document.createElement('button');
        b.type = 'button';
        b.className = `export-item${entry.danger ? ' danger' : ''}`;
        b.setAttribute('role', 'menuitem');
        b.disabled = !!entry.disabled;
        b.innerHTML = `${entry.icon ? `<span data-icon="${entry.icon}"></span>` : ''}<span>${esc(entry.label)}${entry.sub ? `<span class="item-sub">${esc(entry.sub)}</span>` : ''}</span>`;
        b.addEventListener('click', () => { closeMenu(); entry.run?.(); });
        return b;
    }));
    hydrateIcons(menu);
    menu.hidden = false;
    const { width, height } = menu.getBoundingClientRect();
    const r = anchor instanceof Element ? anchor.getBoundingClientRect() : { left: anchor.x, right: anchor.x, top: anchor.y, bottom: anchor.y };
    const left = anchor instanceof Element ? r.right - width : r.left;
    menu.style.left = `${Math.round(Math.min(Math.max(8, left), innerWidth - width - 8))}px`;
    menu.style.top = `${Math.round(r.bottom + 4 + height > innerHeight - 8 ? Math.max(8, r.top - height - 4) : r.bottom + 4)}px`;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    items[0]?.focus();
    const onDown = (e: PointerEvent) => { if (!menu.contains(e.target as Node)) closeMenu(); };
    const onKey = (e: KeyboardEvent) => {
        if (e.key === 'Escape') { e.stopPropagation(); closeMenu(); if (anchor instanceof HTMLElement) anchor.focus(); }
        else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            const i = items.indexOf(document.activeElement as HTMLButtonElement);
            items[(i + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
        }
    };
    setTimeout(() => document.addEventListener('pointerdown', onDown, true));
    document.addEventListener('keydown', onKey, true);
    window.addEventListener('blur', closeMenu, { once: true });
    menuCleanup = () => {
        menu.hidden = true;
        document.removeEventListener('pointerdown', onDown, true);
        document.removeEventListener('keydown', onKey, true);
    };
}

export interface ModalButton { label: string; value: string; primary?: boolean; danger?: boolean }

// Resolves with the clicked button's value, or '' when dismissed. A `field` adds a text input
// whose value is returned for the primary button as `{ value, text }`.
export function modal(opts: { title: string; body?: string; html?: string; buttons: ModalButton[]; field?: { value: string; label: string } }): Promise<{ value: string; text: string }> {
    const dlg = $<HTMLDialogElement>('marsModal');
    $('marsModalTitle').textContent = opts.title;
    const body = $('marsModalBody');
    body.innerHTML = opts.html ?? (opts.body ? `<p>${esc(opts.body)}</p>` : '');
    let input: HTMLInputElement | null = null;
    if (opts.field) {
        input = Object.assign(document.createElement('input'), { className: 'field', value: opts.field.value, autocomplete: 'off', spellcheck: false });
        input.setAttribute('aria-label', opts.field.label);
        // Implicit submission would pick the first (Cancel) button.
        input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); dlg.close(opts.buttons.find((b) => b.primary)?.value ?? ''); } });
        body.append(input);
    }
    $('marsModalFoot').replaceChildren(...opts.buttons.map((b) => {
        const el = Object.assign(document.createElement('button'), { type: 'submit', value: b.value, textContent: b.label });
        el.className = `btn${b.primary ? ' btn-primary' : b.danger ? ' btn-danger' : ' btn-empty'}`;
        return el;
    }));
    dlg.returnValue = '';
    dlg.showModal();
    if (input) { input.focus(); input.select(); }
    else dlg.querySelector<HTMLButtonElement>('.btn-primary, .btn-danger')?.focus();
    return new Promise((resolve) => {
        dlg.addEventListener('close', () => resolve({ value: dlg.returnValue, text: input?.value.trim() ?? '' }), { once: true });
    });
}

export const confirmDanger = async (title: string, body: string, label: string) =>
    (await modal({ title, body, buttons: [{ label: 'Cancel', value: '' }, { label, value: 'ok', danger: true }] })).value === 'ok';

export async function prompt(title: string, label: string, value: string, action = 'Save') {
    const r = await modal({ title, field: { value, label }, buttons: [{ label: 'Cancel', value: '' }, { label: action, value: 'ok', primary: true }] });
    return r.value === 'ok' && r.text ? r.text : null;
}

// Safari keeps the click's user activation only for a clipboard write started synchronously,
// so the write starts at once with promises that settle after the async work.
export async function copyRich(text: Promise<string> | string, html?: Promise<string> | string) {
    const plain = Promise.resolve(text), rich = html === undefined ? null : Promise.resolve(html);
    if (typeof ClipboardItem === 'function' && navigator.clipboard?.write) {
        try {
            const items: Record<string, Promise<Blob>> = { 'text/plain': plain.then((t) => new Blob([t], { type: 'text/plain' })) };
            if (rich) items['text/html'] = rich.then((t) => new Blob([t], { type: 'text/html' }));
            await navigator.clipboard.write([new ClipboardItem(items)]);
            return;
        } catch (err) {
            if (await plain.then(() => false, () => true)) throw err;
        }
    }
    await navigator.clipboard.writeText(await plain);
}

export function download(name: string, data: BlobPart, type: string) {
    const url = URL.createObjectURL(new Blob([data], { type }));
    const a = Object.assign(document.createElement('a'), { href: url, download: name });
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export const fileSafe = (name: string) => name.replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '-').replace(/^[.\s]+|[.\s]+$/g, '').slice(0, 120) || 'Untitled';
