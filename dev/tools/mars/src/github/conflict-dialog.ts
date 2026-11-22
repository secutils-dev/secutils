// Opens a persisted conflict without pushing, overwriting, or inserting Git markers.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { flush, toast } from '../doc/viewer.js';
import { openMergeReview } from '../doc/review-workspace.ts';
import { mergeGitHubText } from './merge.ts';
import { resolveGitHubConflict } from './workspace.ts';

/** Open the current exact local/GitHub variants and offer a safe, explicit resolution. */
export async function showGitHubConflict(id: string) {
    try {
        await flush();
        const current = (await db.readGitHubWorkingCopies([id])).get(id);
        if (!current?.conflict) {
            toast('This conflict is already resolved.');
            return;
        }
        const conflict = { ...current.conflict, local: current.text };
        const parts = mergeGitHubText(conflict.base, conflict.local, conflict.remote);
        openMergeReview(
            app.node(id)?.name ?? 'Document',
            parts.some((part) => !('text' in part)) ? parts : [conflict],
            { local: conflict.local, remote: conflict.remote },
            (text) => resolveGitHubConflict(id, conflict, text),
        );
    } catch {
        toast('Could not open the conflict. Your local edits are still here.');
    }
}
