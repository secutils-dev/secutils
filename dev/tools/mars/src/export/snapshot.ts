// The immutable input of one export. The dialog captures a snapshot once and builds every preview
// and download from it. A later edit, reload, or document switch cannot change an output that the
// reader already saw. Pure: no DOM and no storage access.
import type { ReadingWidth } from './settings.ts';
import { matchCommentBlock } from '../share/merge.ts';

/** A frozen copy of one document at capture time. */
export interface ExportSnapshot {
    /** Library identifier, or null for a document that has no entry. */
    readonly id: string | null;
    readonly name: string;
    /** The complete document text, as the Markdown download writes it. */
    readonly text: string;
    /** The Markdown that the HTML and PDF outputs render (`exportBody(text)`). */
    readonly body: string;
    /** The reading width of the preview at capture time. */
    readonly readingWidth: ReadingWidth;
}

/**
 * The body for reading output: no review comments block, no frontmatter, LF line endings.
 * Deliberately independent of viewer.parseSource: capture must not replace review state.
 * Strip even malformed review metadata from reading output; the .md download keeps it.
 */
export function exportBody(text: string) {
    const block = matchCommentBlock(text);
    return (block ? text.slice(0, block.index) : text)
        .replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '')
        .replace(/\r\n?/g, '\n');
}

/** Creates a frozen snapshot and derives its `body` from `text`. */
export const snapshot = (id: string | null, name: string, text: string, readingWidth: ReadingWidth): ExportSnapshot =>
    Object.freeze({ id, name, text, body: exportBody(text), readingWidth });

/**
 * The cache key of the rendered HTML: two snapshots with the same body share one render.
 * Saved model views live in the body (fence metadata). Temporary navigation never enters it.
 */
export const renderKey = (value: ExportSnapshot) => value.body;
