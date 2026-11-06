import type { ReadingWidth } from './settings.ts';

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
    return text.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '')
        .replace(/(?:^|\r?\n)<!-- su-comments\r?\n[\s\S]*?\r?\n-->\s*$/, '')
        .replace(/\r\n?/g, '\n');
}

export const snapshot = (id: string | null, name: string, text: string, readingWidth: ReadingWidth): ExportSnapshot =>
    Object.freeze({ id, name, text, body: exportBody(text), readingWidth });

// Saved model views live in the body (fence metadata). Temporary navigation never enters it.
export const renderKey = (value: ExportSnapshot) => value.body;
