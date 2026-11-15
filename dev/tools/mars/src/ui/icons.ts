// Preserve the app's typed API; glyphs and rendering are shared with HTML tools.
// The markup comes from the fixed glyph table only, so it is safe for `innerHTML`.
export { ICONS, icon } from '../../../shared/icons.js';
import { ICONS } from '../../../shared/icons.js';
import { icon } from '../../../shared/icons.js';
/** A glyph name of the shared icon table. */
export type IconName = keyof typeof ICONS;

// The existing EUI refresh path sits in the same 16px viewport as the base glyph.
const syncMark = (y: number) =>
    `<g class="tree-sync-mark" transform="translate(5 ${y}) scale(0.375)">${ICONS.refresh}</g>`;
const composite = (paths: string) =>
    `<svg class="eui-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">${paths}</svg>`;

/** Returns a document glyph with an optional static Sync mark inside its single viewport. */
export function documentIcon(synced = false) {
    return synced ? composite(ICONS.document + syncMark(7)) : icon('document');
}

/**
 * Returns one SVG viewport for a folder. `synced` adds a static Sync mark inside the glyph.
 * `preferred` marks the default destination with a star instead, even when it is synced.
 */
export function folderIcon(open: boolean, preferred = false, synced = false) {
    const name = open ? 'folderOpen' : 'folder';
    if (!preferred) return synced ? composite(ICONS[name] + syncMark(open ? 7 : 6)) : icon(name);
    // Both paths share a viewport and rasterization transform at fractional zoom.
    return `<svg class="eui-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">${ICONS[name]}<g class="tree-default-star" transform="translate(5 ${open ? 7 : 6}) scale(0.375)">${ICONS.starFill}</g></svg>`;
}
