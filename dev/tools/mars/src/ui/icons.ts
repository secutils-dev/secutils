// Preserve the app's typed API; glyphs and rendering are shared with HTML tools.
export { ICONS, icon } from '../../../shared/icons.js';
import { ICONS } from '../../../shared/icons.js';
import { icon } from '../../../shared/icons.js';
export type IconName = keyof typeof ICONS;

export function folderIcon(open: boolean, preferred = false) {
    const name = open ? 'folderOpen' : 'folder';
    if (!preferred) return icon(name);
    // Both paths share a viewport and rasterization transform at fractional zoom.
    return `<svg class="eui-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">${ICONS[name]}<g class="tree-default-star" transform="translate(5 ${open ? 7 : 6}) scale(0.375)">${ICONS.starFill}</g></svg>`;
}
