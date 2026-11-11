// Native EuiToolTip / EuiCopy behavior without React or a tooltip dependency.
let tip: HTMLDivElement | null = null, anchor: HTMLElement | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
let describedBy: string | null = null, installed = false;
const ICON_BUTTON = '.icon-btn, .su-header-btn, .su-dialog-close, .tree-more, .tree-connection, button[aria-label][title]';

export function hideTooltip() {
    clearTimeout(timer);
    if (anchor) {
        if (describedBy === null) anchor.removeAttribute('aria-describedby'); else anchor.setAttribute('aria-describedby', describedBy);
    }
    if (tip?.matches(':popover-open')) tip.hidePopover();
    tip?.remove(); tip = null; anchor = null; describedBy = null;
}
function position() {
    if (!tip || !anchor?.isConnected) { hideTooltip(); return; }
    const rect = anchor.getBoundingClientRect(), box = tip.getBoundingClientRect();
    const left = Math.max(8, Math.min(rect.left + rect.width / 2 - box.width / 2, innerWidth - box.width - 8));
    const below = rect.top - box.height - 8 < 8;
    tip.dataset.side = below ? 'bottom' : 'top';
    tip.style.left = `${Math.round(left)}px`;
    tip.style.top = `${Math.round(below ? Math.min(innerHeight - box.height - 8, rect.bottom + 8) : rect.top - box.height - 8)}px`;
    tip.style.setProperty('--tooltip-arrow-x', `${Math.max(8, Math.min(box.width - 8, rect.left + rect.width / 2 - left))}px`);
}
export function showTooltip(element: HTMLElement, content: string, duration = 0) {
    if (!element.isConnected) return;
    hideTooltip(); anchor = element; describedBy = element.getAttribute('aria-describedby');
    tip = document.createElement('div'); tip.id = 'marsTooltip'; tip.className = 'su-tooltip'; tip.textContent = content;
    tip.setAttribute('role', 'tooltip'); tip.setAttribute('popover', 'manual');
    const label = element.getAttribute('aria-label') ?? element.textContent?.trim() ?? '';
    if (label.includes(content)) tip.setAttribute('aria-hidden', 'true');
    else element.setAttribute('aria-describedby', `${describedBy ? describedBy + ' ' : ''}${tip.id}`);
    (element.closest('dialog') ?? document.body).append(tip);
    tip.showPopover(); position();
    if (duration) timer = setTimeout(hideTooltip, duration);
}
export const copyFeedback = (element: HTMLElement, text = 'Copied') => showTooltip(element, text, 1800);

function migrate(element: Element) {
    if (!(element instanceof HTMLElement) || !element.matches(ICON_BUTTON)) return;
    const title = element.getAttribute('title') ?? (!element.hasAttribute('data-tooltip') ? element.getAttribute('aria-label') : null);
    if (!title) return;
    if (!element.hasAttribute('aria-label')) element.setAttribute('aria-label', title);
    element.dataset.tooltip = title; element.removeAttribute('title');
}
export function installTooltips() {
    if (installed) return; installed = true;
    for (const element of document.querySelectorAll(ICON_BUTTON)) migrate(element);
    new MutationObserver((records) => {
        for (const record of records) {
            if (record.type === 'attributes') migrate(record.target as Element);
            for (const node of record.addedNodes) if (node instanceof Element) {
                migrate(node); for (const element of node.querySelectorAll(ICON_BUTTON)) migrate(element);
            }
        }
        if (anchor && !anchor.isConnected) hideTooltip();
    }).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['title', 'aria-label'] });
    const target = (event: Event) => (event.target as Element)?.closest<HTMLElement>('[data-tooltip]');
    const enter = (event: Event) => {
        const element = target(event); if (!element || anchor === element) return;
        clearTimeout(timer); timer = setTimeout(() => showTooltip(element, element.dataset.tooltip!), 250);
    };
    const leave = (event: MouseEvent | FocusEvent) => {
        const element = target(event);
        if (element && !element.contains(event.relatedTarget as Node | null)
            && !element.matches(':hover, :focus-within')) hideTooltip();
    };
    document.addEventListener('pointerover', enter); document.addEventListener('focusin', enter);
    document.addEventListener('pointerout', leave); document.addEventListener('focusout', leave);
    document.addEventListener('keydown', event => { if (event.key === 'Escape') hideTooltip(); }, true);
    document.addEventListener('scroll', hideTooltip, true); window.addEventListener('resize', position);
    window.addEventListener('pagehide', hideTooltip);
}
