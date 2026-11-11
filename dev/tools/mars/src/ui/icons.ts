// Preserve the app's typed API; glyphs and rendering are shared with HTML tools.
// The markup comes from the fixed glyph table only, so it is safe for `innerHTML`.
export { ICONS, icon } from '../../../shared/icons.js';
import { ICONS } from '../../../shared/icons.js';
import { icon } from '../../../shared/icons.js';
/** A glyph name of the shared icon table. */
export type IconName = keyof typeof ICONS;

/**
 * Returns the SVG markup of a folder icon. `preferred` adds the star overlay that marks the
 * default folder for incoming documents.
 */
export function folderIcon(open: boolean, preferred = false) {
    const name = open ? 'folderOpen' : 'folder';
    if (!preferred) return icon(name);
    // Both paths share a viewport and rasterization transform at fractional zoom.
    return `<svg class="eui-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">${ICONS[name]}<g class="tree-default-star" transform="translate(5 ${open ? 7 : 6}) scale(0.375)">${ICONS.starFill}</g></svg>`;
}
