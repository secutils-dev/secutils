// Native GitHub import form. The PAT is never submitted to an app endpoint.
import { node, state } from '../app.ts';
import { flush, toast } from '../doc/viewer.js';
import { onSideChange } from '../store/db.ts';
import { icon } from '../ui/icons.ts';
import { esc, modal } from '../ui/ui.ts';
import { SyncDialog } from '../sync/dialog.ts';
import { githubSource } from './api.ts';
import { manageGitHubCredentials } from './credential-dialog.ts';
import { gitHubCredentials, type CredentialLabel } from './credential-store.ts';
import { openGitHubExclusions } from './exclusion-dialog.ts';
import { reviewGitHubChanges } from './pending-dialog.ts';
import { connectGitHub, disconnectGitHub, hasFolderGitHubToken, hasGitHubSession, syncGitHub } from './workspace.ts';

/** Import a repository or manage an existing folder's GitHub Sync. */
export function openGitHubDialog(id?: string) {
    if (document.getElementById('githubImportDialog')) return;
    let source = id ? node(id)?.github : undefined;
    const pending = id ? node(id)?.githubReview : undefined;
    if (id && !source) return;
    const dialog = document.createElement('dialog');
    dialog.id = 'githubImportDialog';
    dialog.className = 'su-dialog mars-modal sync-dialog';
    dialog.setAttribute('aria-labelledby', 'githubImportTitle');
    dialog.innerHTML = `<header class="su-dialog-header"><h2 id="githubImportTitle">${source ? 'Sync' : 'Import from GitHub'}</h2><button type="button" class="su-dialog-close" id="githubCancel" aria-label="Close">${icon('cross')}</button></header>
        <form method="dialog" aria-labelledby="githubImportTitle">
            <div class="su-dialog-body github-fields">
                ${source ? `<p><strong>${esc(source.repository)}</strong> · ${esc(source.branch)} <span class="permission-badge">${source.write ? 'Read and write' : 'Read-only'}</span></p>` : ''}
                ${
                    source
                        ? ''
                        : `
                <label for="githubRepo">Repository (required)</label>
                <input class="field" id="githubRepo" name="repository" required maxlength="170" autofocus autocapitalize="none" spellcheck="false" placeholder="owner/repository">
                <label for="githubBranch">Branch</label>
                <input class="field" id="githubBranch" name="branch" maxlength="255" autocapitalize="none" spellcheck="false" placeholder="Default branch">`
                }
                <label for="githubCredential">GitHub credential</label>
                <div class="github-credential-picker">
                    <select class="select" id="githubCredential" name="credential" ${source ? 'autofocus' : ''}></select>
                    <button type="button" class="icon-btn icon-btn-s" id="githubManageCredentials" aria-label="Manage credentials" data-tooltip="Manage credentials" aria-haspopup="dialog">${icon('key')}</button>
                </div>
                <label><input id="githubWrite" name="write" type="checkbox" disabled> Sync my edits back to GitHub</label>
                <p id="githubImportError" class="sync-error" role="alert" hidden></p>
                ${source ? `<div class="github-sync-actions"><button type="button" class="btn" id="githubExclusions">${icon('eyeSlash')}Manage exclusions</button>${pending?.pending ? `<button type="button" class="btn" id="githubReview">Review pending changes</button>` : ''}</div>` : ''}
            </div>
            <footer class="mars-modal-foot">${source ? '<button class="btn btn-empty sync-footer-left" type="button" id="githubDisconnect">Disconnect this device</button>' : ''}<button class="btn btn-primary" type="submit">${source ? 'Sync now' : 'Import folder'}</button></footer>
        </form>`;
    const controller = new AbortController();
    const input = (name: string) => dialog.querySelector<HTMLInputElement>(`#${name}`)!;
    const write = input('githubWrite');
    const picker = dialog.querySelector<HTMLSelectElement>('#githubCredential')!;
    const manage = dialog.querySelector<HTMLButtonElement>('#githubManageCredentials')!;
    const problem = dialog.querySelector<HTMLElement>('#githubImportError')!;
    const submit = dialog.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    const submitLabel = submit.textContent;
    const targetSource = () => source ?? githubSource(input('githubRepo').value, input('githubBranch').value);
    let labels: CredentialLabel[] = [];
    let selection = 'public';
    let chosen = false;
    let legacyAvailable = false;
    let loadingCredentials = true;
    let writeChoice = !!source?.write;
    const owner = () => {
        try {
            return targetSource().repository.split('/')[0].toLowerCase();
        } catch {
            return '';
        }
    };
    const updatePicker = () => {
        const org = owner();
        const matches = labels.filter((label) => org && label.owner.toLowerCase() === org);
        const rest = labels.filter((label) => !matches.includes(label));
        let previous = source;
        if (!id) {
            try {
                const { repository, branch } = targetSource();
                const imports = state.nodes.filter(
                    (entry) =>
                        entry.github?.repository.toLowerCase() === repository.toLowerCase() &&
                        (!branch || entry.github.branch === branch),
                );
                if (imports.length === 1) previous = imports[0].github;
            } catch {
                /* The repository is not complete yet. */
            }
        }
        if (!chosen) selection = previous?.credentialId ?? (legacyAvailable ? 'legacy' : 'public');
        picker.replaceChildren(new Option('None (public read-only)', 'public'));
        if (legacyAvailable) picker.add(new Option('This folder’s token', 'legacy'));
        if (selection !== 'public' && selection !== 'legacy' && !labels.some((label) => label.id === selection))
            picker.add(new Option('Credential unavailable', selection));
        for (const [name, group] of [
            [`Suggested for ${org}`, matches],
            ['Saved credentials', rest],
        ] as const) {
            if (!group.length) continue;
            const options = document.createElement('optgroup');
            options.label = name;
            for (const label of group)
                options.append(new Option(label.owner ? `${label.name} · ${label.owner}` : label.name, label.id));
            picker.append(options);
        }
        picker.value = selection;
        updateWrite();
    };
    const updateWrite = () => {
        write.disabled = selection !== 'legacy' && !labels.some((label) => label.id === selection);
        write.checked = !write.disabled && writeChoice;
    };
    const loadCredentials = async (selected?: string) => {
        try {
            const [next, saved] = await Promise.all([
                gitHubCredentials(),
                id && !source?.credentialId ? hasFolderGitHubToken(id) : false,
            ]);
            if (!dialog.open) return;
            labels = next;
            legacyAvailable = saved;
            if (selected) {
                selection = selected;
                chosen = true;
            }
            updatePicker();
        } catch {
            if (!dialog.open) return;
            problem.textContent = 'Could not read saved GitHub credentials.';
            problem.hidden = false;
        } finally {
            if (loadingCredentials) {
                loadingCredentials = false;
                submit.disabled = false;
            }
        }
    };
    onSideChange((kind) => {
        if (kind === 'github-credential') void loadCredentials();
    }, controller.signal);
    picker.addEventListener('change', () => {
        selection = picker.value;
        chosen = true;
        problem.hidden = true;
        updateWrite();
    });
    manage.addEventListener('click', async () => {
        try {
            const selected = await manageGitHubCredentials({ owner: owner(), folder: id });
            if (id && selected && node(id)?.github?.credentialId === selected) source = node(id)?.github;
            if (selected) problem.hidden = true;
            await loadCredentials(selected);
        } catch {
            problem.textContent = 'Could not open saved GitHub credentials.';
            problem.hidden = false;
        }
    });
    write.addEventListener('change', () => {
        writeChoice = write.checked;
    });
    dialog.querySelector('#githubRepo')?.addEventListener('input', updatePicker);
    dialog.querySelector('#githubBranch')?.addEventListener('input', updatePicker);
    dialog.addEventListener(
        'close',
        () => {
            controller.abort();
            dialog.remove();
        },
        { once: true },
    );
    dialog.querySelector('#githubCancel')!.addEventListener('click', () => dialog.close());
    const exclusions = dialog.querySelector<HTMLButtonElement>('#githubExclusions');
    exclusions?.addEventListener('click', () => openGitHubExclusions(id!));
    dialog.querySelector('#githubReview')?.addEventListener('click', () => {
        dialog.close();
        if (id) void reviewGitHubChanges(id);
    });
    const disconnect = dialog.querySelector<HTMLButtonElement>('#githubDisconnect');
    disconnect?.addEventListener('click', async () => {
        disconnect.disabled = submit.disabled = true;
        problem.hidden = true;
        try {
            const answer = await modal({
                title: 'Disconnect this device?',
                body: 'This folder and its documents stay on this device as independent copies. They will no longer send or receive changes. The GitHub repository and other devices are unchanged.',
                buttons: [
                    { label: 'Cancel', value: '' },
                    { label: 'Disconnect', value: 'disconnect' },
                ],
                signal: controller.signal,
            });
            if (answer.value !== 'disconnect') return;
            await disconnectGitHub(id!);
            if (dialog.open) dialog.close();
            toast('Sync is off. Your folder and documents are still here.');
        } catch {
            if (dialog.open) {
                problem.textContent = 'Could not disconnect this folder. Your documents are still here.';
                problem.hidden = false;
            }
        } finally {
            disconnect.disabled = submit.disabled = false;
        }
    });
    dialog.querySelector('form')!.addEventListener('submit', async (event) => {
        event.preventDefault();
        if (submit.disabled) return;
        submit.disabled = true;
        picker.disabled = manage.disabled = true;
        if (exclusions) exclusions.disabled = true;
        if (disconnect) disconnect.disabled = true;
        problem.hidden = true;
        let download: SyncDialog | undefined;
        try {
            if (id && node(id)?.github?.authRevision !== source?.authRevision)
                throw new Error('This folder changed in another tab. Reopen Sync to use its current settings.');
            const target = targetSource();
            const selected = selection;
            const legacy = selected === 'legacy';
            const named = !legacy && selected !== 'public';
            if (named && !labels.some((label) => label.id === selected))
                throw new Error('This credential is no longer available. Choose another credential.');
            if (
                id &&
                source &&
                hasGitHubSession(id) &&
                write.checked === source.write &&
                (named
                    ? selected === source.credentialId
                    : !source.credentialId && (legacy || !(source.authenticated ?? source.write)))
            ) {
                submit.dataset.loading = 'true';
                submit.setAttribute('aria-busy', 'true');
                submit.innerHTML = '<span class="su-spinner" aria-hidden="true"></span>Syncing…';
                await flush();
                const message = await syncGitHub(id, false, true);
                if (dialog.open) {
                    dialog.close();
                    if (message) toast(message);
                }
                return;
            }
            if (!dialog.open) return;
            download = new SyncDialog('githubDownloadDialog');
            const current = download;
            void current
                .screen({
                    title: 'Downloading GitHub folder',
                    html: '<p>Preparing your repository folder.</p><progress class="sync-download-progress" aria-label="GitHub download" aria-describedby="githubDownloadStatus"></progress><p id="githubDownloadStatus" role="status" aria-atomic="true">Reading folder details…</p><p class="sync-fineprint">Keep this tab open. The folder will appear with its complete hierarchy when the download finishes.</p>',
                    buttons: [{ label: 'Cancel', value: 'cancel' }],
                })
                .then(() => current.close());
            const meter = current.element.querySelector('progress')!,
                status = current.element.querySelector('#githubDownloadStatus')!;
            await connectGitHub(
                target,
                named ? { credentialId: selected } : legacy ? { keepFolderToken: true } : { publicOnly: true },
                write.checked,
                id,
                AbortSignal.any([controller.signal, current.signal]),
                (done, total, stage) => {
                    if (!current.open) return;
                    if (total) {
                        meter.max = total;
                        meter.value = done;
                    }
                    status.textContent =
                        stage === 'save'
                            ? 'Saving folder hierarchy…'
                            : stage === 'details'
                              ? 'Reading folder details…'
                              : `${done} of ${total} ${total === 1 ? 'file' : 'files'} downloaded`;
                },
            );
            current.close();
            dialog.close();
        } catch (error) {
            const cancelled = download?.signal.aborted;
            download?.close();
            if (dialog.open && !cancelled) {
                problem.textContent =
                    error instanceof TypeError
                        ? 'Could not reach GitHub. Check your connection and try again.'
                        : error instanceof Error
                          ? error.message
                          : 'Could not open the GitHub folder.';
                problem.hidden = false;
            }
        } finally {
            submit.disabled = false;
            picker.disabled = manage.disabled = false;
            if (exclusions) exclusions.disabled = false;
            if (disconnect) disconnect.disabled = false;
            if (submit.dataset.loading) {
                delete submit.dataset.loading;
                submit.removeAttribute('aria-busy');
                submit.textContent = submitLabel;
            }
        }
    });
    document.body.append(dialog);
    updatePicker();
    submit.disabled = true;
    dialog.showModal();
    void loadCredentials();
}
