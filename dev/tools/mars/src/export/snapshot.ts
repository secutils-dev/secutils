import type { ReadingWidth } from './settings.ts';
import { matchCommentBlock } from '../share/merge.ts';

export interface ExportSnapshot {
    readonly id: string | null;
    readonly name: string;
    readonly text: string;
    readonly body: string;
    readonly readingWidth: ReadingWidth;
}

// Deliberately independent of viewer.parseSource: capture must not replace review state.
// Strip even malformed review metadata from reading output; the .md download keeps it.
export function exportBody(text: string) {
    const block = matchCommentBlock(text);
    return (block ? text.slice(0, block.index) : text)
        .replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '')
        .replace(/\r\n?/g, '\n');
}

export const snapshot = (id: string | null, name: string, text: string, readingWidth: ReadingWidth): ExportSnapshot =>
    Object.freeze({ id, name, text, body: exportBody(text), readingWidth });

// Saved model views live in the body (fence metadata). Temporary navigation never enters it.
export const renderKey = (value: ExportSnapshot) => value.body;
