// Rename feedback shared by the sidebar and the document title. Name changes use the app's
// persistence queue, not the Markdown undo stack. Pending text is saved before either write.
import * as app from '../app.ts';
import * as viewer from '../doc/viewer.js';
import { githubRoot } from '../github/binding.ts';
import { changeGitHubFiles } from '../github/files.ts';

/** Renames a node and offers Undo in the toast. Failures show a fixed code and retain the saved name. */
export async function renameNode(id: string, name: string) {
    try {
        const n = app.node(id),
            repo = githubRoot(app.state.nodes, n);
        if (n && repo && n.id !== repo.id) {
            if (!repo.github?.write || n.name === name.trim()) return;
            await changeGitHubFiles(repo.id, { kind: 'rename', id, name });
            return;
        }
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
    } catch (error) {
        const n = app.node(id);
        viewer.toast(
            n && githubRoot(app.state.nodes, n) && error instanceof Error
                ? error.message
                : 'rename-failed: Could not save the name. Try again.',
        );
    }
}
