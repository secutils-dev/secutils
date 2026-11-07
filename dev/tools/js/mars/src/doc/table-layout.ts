import { metadataWidths } from './table.ts';

export function setTableWidths(table: HTMLTableElement, widths: number[] | null) {
    table.querySelector(':scope > colgroup[data-su-table]')?.remove();
    table.classList.toggle('su-sized-table', !!widths);
    if (!widths) return;
    const group = document.createElement('colgroup');
    group.dataset.suTable = '';
    for (const width of widths) {
        const col = document.createElement('col'); col.style.width = `${width * 100}%`; group.append(col);
    }
    table.prepend(group);
}

// Read layout from adjacent inert Markdown comments, in both fresh previews and export rendering.
export function applyTableLayouts(root: HTMLElement, forExport: boolean) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);
    const comments: Comment[] = [];
    while (walker.nextNode()) comments.push(walker.currentNode as Comment);
    for (const comment of comments) {
        const match = comment.data.match(/^\s*su-table\s+([^\r\n]*?)\s*$/);
        if (!match) continue;
        const parent = comment.parentElement;
        const block = parent?.matches('.su-block') && [...parent.childNodes].every((n) => n.nodeType === Node.COMMENT_NODE || (n.nodeType === Node.TEXT_NODE && !n.textContent?.trim())) ? parent : null;
        let next: ChildNode | null = (block ?? comment).nextSibling;
        while (next?.nodeType === Node.TEXT_NODE && !next.textContent?.trim()) next = next.nextSibling;
        const element = next instanceof HTMLElement ? next : null;
        const table = element instanceof HTMLTableElement ? element
            : element?.matches('.su-block') && element.firstElementChild instanceof HTMLTableElement ? element.firstElementChild : null;
        if (!table) continue;
        const widths = metadataWidths(match[1], table.rows[0]?.cells.length ?? 0);
        setTableWidths(table, widths);
        if (block && !forExport && element) {
            element.dataset.start = block.dataset.start;
            block.remove();
        } else if (forExport) comment.remove();
    }
}
