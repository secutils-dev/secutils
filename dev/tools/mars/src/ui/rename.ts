// Rename feedback shared by the sidebar and the document title. Name changes use the app's
// persistence queue, not the Markdown undo stack. Pending text is saved before either write.
import * as app from '../app.ts';
import * as viewer from '../doc/viewer.js';

/** Renames a node and offers Undo in the toast. Failures show a fixed code and retain the saved name. */
export async function renameNode(id: string, name: string) {
    try {
        await viewer.flush();
        const undo = await app.rename(id, name);
        if (!undo) return;
        const run = async () => {
            try {
                await viewer.flush();
                viewer.toast((await undo()) ? 'Rename undone' : 'Rename changed again. Nothing was undone.');
            } catch {
                viewer.toast('rename-undo-failed: Could not undo the rename. Try again.', { label: 'Undo', run });
            }
        };
        viewer.toast(`Renamed to "${app.node(id)!.name}"`, { label: 'Undo', run });
    } catch {
        viewer.toast('rename-failed: Could not save the name. Try again.');
    }
}
