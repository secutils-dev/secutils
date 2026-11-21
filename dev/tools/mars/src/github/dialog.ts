// Native GitHub import form. The PAT is never submitted to an app endpoint.
import { node } from '../app.ts';
import { esc } from '../ui/ui.ts';
import { githubSource } from './api.ts';
import { connectGitHub, githubStatus } from './workspace.ts';

/** Import a branch or reconnect to one already selected in the library. */
export function openGitHubDialog(id?: string) {
    if (document.getElementById('githubImportDialog')) return;
    const source = id ? node(id)?.github : undefined;
    if (id && !source) return;
    const dialog = document.createElement('dialog');
    dialog.id = 'githubImportDialog';
    dialog.className = 'su-dialog mars-modal sync-dialog';
    dialog.setAttribute('aria-labelledby', 'githubImportTitle');
    dialog.innerHTML = `<header class="su-dialog-header"><h2 id="githubImportTitle">${source ? 'Reconnect GitHub' : 'Import from GitHub'}</h2></header>
        <form method="dialog" aria-labelledby="githubImportTitle">
            <div class="su-dialog-body github-fields">
                <p>${source ? esc(githubStatus(id!)) : 'Read supported files from a GitHub branch. GitHub controls access. The folder does not expire.'}</p>
                <label for="githubRepo">Repository (required)</label>
                <input class="field" id="githubRepo" name="repository" required maxlength="170" value="${esc(source?.repository ?? '')}" ${source ? 'readonly' : 'autofocus'} autocapitalize="none" spellcheck="false" placeholder="owner/repository">
                <label for="githubBranch">Branch</label>
                <input class="field" id="githubBranch" name="branch" maxlength="255" value="${esc(source?.branch ?? '')}" ${source ? 'readonly' : ''} autocapitalize="none" spellcheck="false" placeholder="Default branch">
                <label for="githubToken">Fine-grained personal access token</label>
                <input class="field" id="githubToken" type="password" maxlength="512" aria-describedby="githubTokenHelp" ${source ? 'autofocus' : ''}>
                <p id="githubTokenHelp" class="sync-fineprint">Leave blank for public read-only access. For private reads, grant Contents read. To publish text edits, supply Contents read/write and choose Allow commits. The token stays in this tab.</p>
                <label><input id="githubWrite" name="write" type="checkbox" ${source?.write ? 'checked' : ''}> Allow commits to the selected branch</label>
                <p class="sync-fineprint">Use a test branch for this preview. File additions, moves, and deletions made in MARS are not published.</p>
                <p id="githubImportStatus" class="sync-notice" role="status"></p>
            </div>
            <footer class="mars-modal-foot"><button class="btn" type="button" id="githubCancel">Close</button><button class="btn btn-primary" type="submit">${source ? 'Reconnect' : 'Import folder'}</button></footer>
        </form>`;
    const controller = new AbortController();
    const input = (name: string) => dialog.querySelector<HTMLInputElement>(`#${name}`)!;
    const token = input('githubToken'),
        write = input('githubWrite');
    const status = dialog.querySelector<HTMLElement>('#githubImportStatus')!;
    const submit = dialog.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    token.addEventListener('input', () => token.setCustomValidity(''));
    write.addEventListener('change', () => token.setCustomValidity(''));
    dialog.addEventListener(
        'close',
        () => {
            controller.abort();
            token.value = '';
            dialog.remove();
        },
        { once: true },
    );
    dialog.querySelector('#githubCancel')!.addEventListener('click', () => dialog.close());
    dialog.querySelector('form')!.addEventListener('submit', async (event) => {
        event.preventDefault();
        const secret = token.value.trim();
        if (write.checked && !secret) {
            token.setCustomValidity('Add a token to allow commits.');
            token.reportValidity();
            return;
        }
        submit.disabled = true;
        status.textContent = 'Reading the GitHub branch…';
        try {
            status.textContent = await connectGitHub(
                githubSource(input('githubRepo').value, input('githubBranch').value),
                secret,
                write.checked,
                id,
                controller.signal,
            );
            if (dialog.open) dialog.close();
        } catch (error) {
            if (dialog.open)
                status.textContent = error instanceof Error ? error.message : 'Could not open the GitHub folder.';
        } finally {
            submit.disabled = false;
        }
    });
    document.body.append(dialog);
    dialog.showModal();
}
