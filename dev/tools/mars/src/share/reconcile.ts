// Body selection for incoming copies is shared by individual opens and batch re-imports.
// Review comments do not make a body conflict. No function reads storage or changes its inputs.
import { splitComments } from './merge.ts';

/** The body used for source comparison, without review threads or line-ending differences. */
export const sourceBody = (text: string) => splitComments(text).body.replace(/\r\n?/g, '\n').trim();

/** Chooses the unchanged side, or requests a decision when both bodies changed or the source is unknown. */
export function incomingChoice(local: string, incoming: string, opened?: string): 'mine' | 'theirs' | 'conflict' {
    if (sourceBody(local) === sourceBody(incoming)) return 'mine';
    if (opened !== undefined && sourceBody(opened) === sourceBody(incoming)) return 'mine';
    if (opened !== undefined && sourceBody(opened) === sourceBody(local)) return 'theirs';
    return 'conflict';
}
