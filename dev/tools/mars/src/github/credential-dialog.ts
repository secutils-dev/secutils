// Manage local credential labels and sealed tokens. Token fields are never filled from storage.
import * as app from '../app.ts';
import { SyncDialog } from '../sync/dialog.ts';
import { icon } from '../ui/icons.ts';
import { esc, modal } from '../ui/ui.ts';
import {
    gitHubCredentials,
    removeGitHubCredential,
    saveGitHubCredential,
    type CredentialLabel,
} from './credential-store.ts';
import { hasFolderGitHubToken, refreshGitHubCredential, reuseGitHubToken } from './workspace.ts';

const MANAGER = 'githubCredentialManager';
const EDITOR = 'githubCredentialEditor';

// Add or edit a local credential, or reuse an existing folder token without displaying its value.
async function editGitHubCredential({
    owner = '',
    current,
    folder,
}: {
    owner?: string;
    current?: CredentialLabel;
    folder?: string;
} = {}) {
    if (document.getElementById(EDITOR)) return;
    const ui = new SyncDialog(EDITOR);
    const title = folder ? 'Save a reusable credential' : current ? 'Edit GitHub credential' : 'Add GitHub credential';
    try {
        return await new Promise<string | undefined>((resolve) => {
            const signal = ui.signal;
            signal.addEventListener('abort', () => resolve(undefined), { once: true });
            let saving = false;
            const save = async () => {
                if (saving || !form.reportValidity()) return;
                saving = true;
                const label = { id: current?.id, name: name.value, owner: ownerInput.value };
                const secret = token?.value ?? '';
                if (token) token.value = '';
                ui.busy('Saving credential…');
                try {
                    const id = folder
                        ? await reuseGitHubToken(folder, label)
                        : await saveGitHubCredential(label, secret);
                    void refreshGitHubCredential(id).catch(() => {});
                    resolve(id);
                    ui.close();
                } catch (error) {
                    ui.idle();
                    problem.textContent =
                        error instanceof Error ? error.message : 'Could not save the GitHub credential.';
                } finally {
                    saving = false;
                }
            };
            void ui.screen({
                title,
                html: `<form method="dialog" class="github-credential-fields" aria-label="GitHub credential">
                    <label for="githubCredentialName">Credential name</label>
                    <input id="githubCredentialName" class="field" name="label" required maxlength="100" placeholder="Acme docs" autocomplete="off">
                    <label for="githubCredentialOwner">GitHub owner (optional)</label>
                    <input id="githubCredentialOwner" class="field" name="owner" maxlength="39" aria-describedby="githubCredentialOwnerHelp" autocapitalize="none" spellcheck="false" autocomplete="off">
                    <p id="githubCredentialOwnerHelp" class="sync-fineprint">Enter an organization or account name to help find this credential in the picker.</p>
                    ${
                        folder
                            ? '<p class="sync-fineprint">MARS will use this credential for the current folder. You can also select it for other folders.</p>'
                            : `<label for="githubCredentialToken">${current ? 'Replacement token (optional)' : 'Fine-grained personal access token'}</label>
                    <input id="githubCredentialToken" class="field" name="token" type="password" ${current ? '' : 'required'} maxlength="512" autocomplete="off" aria-describedby="githubCredentialTokenHelp">
                    <p id="githubCredentialTokenHelp" class="sync-fineprint">${current ? 'A replacement token applies to every folder that uses this credential. Leave blank to keep the current token.' : 'On GitHub, limit the token to the repositories you need. Under <strong>Repository permissions → Contents</strong>, choose <strong>Read-only</strong> or <strong>Read and write</strong>.'}</p>`
                    }
                    <p class="sync-error" role="alert"></p>
                    <button type="submit" hidden>Save</button>
                </form>`,
                buttons: [
                    { label: 'Cancel', value: 'cancel', run: () => ui.close() },
                    { label: 'Save credential', value: 'save', primary: true, run: () => form.requestSubmit() },
                ],
            });
            const form = ui.element.querySelector('form')!;
            const name = form.querySelector<HTMLInputElement>('#githubCredentialName')!;
            const ownerInput = form.querySelector<HTMLInputElement>('#githubCredentialOwner')!;
            const token = form.querySelector<HTMLInputElement>('#githubCredentialToken');
            const problem = form.querySelector('.sync-error')!;
            name.value = current?.name ?? '';
            ownerInput.value = current?.owner ?? owner;
            form.addEventListener('submit', (event) => {
                event.preventDefault();
                void save();
            });
            signal.addEventListener(
                'abort',
                () => {
                    if (token) token.value = '';
                },
                { once: true },
            );
            name.focus();
        });
    } finally {
        ui.close();
    }
}

/** Manage local credentials. Returns a newly added credential ID for the caller to select. */
export async function manageGitHubCredentials({ owner = '', folder }: { owner?: string; folder?: string } = {}) {
    if (document.getElementById(MANAGER)) return;
    const ui = new SyncDialog(MANAGER);
    let issue = '';
    let added: string | undefined;
    try {
        while (ui.open) {
            const labels = await gitHubCredentials();
            const legacy = folder ? app.node(folder)?.github : undefined;
            const canReuse = folder && legacy && !legacy.credentialId && (await hasFolderGitHubToken(folder));
            const counts = new Map<string, number>();
            for (const { github } of app.state.nodes)
                if (github?.credentialId) counts.set(github.credentialId, (counts.get(github.credentialId) ?? 0) + 1);
            const rows = labels.map((label) => {
                const count = counts.get(label.id) ?? 0;
                return `<li class="github-credential-row"><div><strong>${esc(label.name)}</strong><div class="sync-fineprint">${label.owner ? `${esc(label.owner)} · ` : ''}${count} ${count === 1 ? 'folder' : 'folders'}</div></div><div class="github-credential-actions">
                    <button type="button" class="icon-btn" data-sync-action="edit:${esc(label.id)}" data-tooltip="Edit ${esc(label.name)}" aria-label="Edit ${esc(label.name)}">${icon('pencil')}</button>
                    <button type="button" class="icon-btn" data-sync-action="remove:${esc(label.id)}" data-tooltip="Remove ${esc(label.name)}" aria-label="Remove ${esc(label.name)}">${icon('trash')}</button></div></li>`;
            });
            const { value } = await ui.screen({
                title: 'GitHub credentials',
                html: `<p>Tokens stay in this browser. MARS sends them directly to GitHub and keeps them out of links and backups.</p>
                    ${rows.length ? `<ul class="github-credential-list">${rows.join('')}</ul>` : '<p class="sync-fineprint">No saved GitHub credentials yet.</p>'}
                    ${canReuse ? '<section class="github-legacy-credential"><p class="sync-fineprint">This folder uses its own token. Save it as a credential to reuse it.</p><button type="button" class="btn btn-sm" data-sync-action="reuse">Save as reusable…</button></section>' : ''}
                    ${issue ? `<p class="sync-error" role="alert">${esc(issue)}</p>` : ''}`,
                buttons: [{ label: 'Add credential', value: 'add', primary: true }],
            });
            if (value === 'close') break;
            issue = '';
            try {
                if (value === 'add') added = (await editGitHubCredential({ owner })) ?? added;
                if (value === 'reuse' && folder) added = (await editGitHubCredential({ owner, folder })) ?? added;
                const [action, id] = value.split(':');
                const current = labels.find((label) => label.id === id);
                if (!current) continue;
                if (action === 'edit') await editGitHubCredential({ current });
                if (action === 'remove') {
                    const answer = await modal({
                        title: `Remove ${current.name}?`,
                        body: 'This removes the token from this browser and pauses Sync for the folders that use it. Local files and pending edits stay. The token is not revoked on GitHub.',
                        buttons: [
                            { label: 'Cancel', value: '' },
                            { label: 'Remove credential', value: 'remove', danger: true },
                        ],
                        signal: ui.signal,
                    });
                    if (answer.value !== 'remove') continue;
                    await removeGitHubCredential(id);
                    if (added === id) added = undefined;
                    await refreshGitHubCredential(id);
                }
            } catch (error) {
                issue = error instanceof Error ? error.message : 'Could not update the GitHub credential.';
            }
        }
    } finally {
        ui.close();
    }
    return added;
}
