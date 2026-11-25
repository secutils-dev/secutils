// GitHub import and connection dialogs. Only the browser's GitHub client uses a PAT.
import { node, state } from '../app.ts';
import { flush, toast } from '../doc/viewer.js';
import { onSideChange } from '../store/db.ts';
import { hideTooltip } from '../ui/feedback.ts';
import { icon } from '../ui/icons.ts';
import { esc } from '../ui/ui.ts';
import { SyncDialog, syncBackButton, syncManagement } from '../sync/dialog.ts';
import { bindScheduleFields, scheduleFields } from '../sync/schedule-dialog.ts';
import { githubSchedule, githubScheduleKey, readSchedule, type SyncSchedule } from '../sync/schedule.ts';
import { githubSource } from './api.ts';
import { manageGitHubCredentials } from './credential-dialog.ts';
import { gitHubCredentials, type CredentialLabel } from './credential-store.ts';
import { openGitHubExclusions } from './exclusion-dialog.ts';
import { reviewGitHubChanges } from './pending-dialog.ts';
import {
    connectGitHub,
    disconnectGitHub,
    gitHubScheduleOptions,
    hasFolderGitHubToken,
    saveGitHubSettings,
    syncGitHub,
    type GitHubSettings,
} from './workspace.ts';

/** Import a branch, or open the status and actions of an existing GitHub connection. */
export function openGitHubDialog(id?: string) {
    if (id) {
        if (!document.getElementById('githubSyncDialog')) void githubStatusDialog(id);
    } else void githubConfiguration();
}

async function githubStatusDialog(id: string) {
    if (!node(id)?.github) return;
    const ui = new SyncDialog('githubSyncDialog');
    let problem = '';
    try {
        while (ui.open) {
            const folder = node(id),
                source = folder?.github;
            if (!source) return;
            const action = await ui.screen({
                title: 'Sync',
                settings: true,
                html: `<p class="github-sync-source"><strong>${esc(source.repository)}</strong> · ${esc(source.branch)} <span class="permission-badge">${source.write ? 'Read and write' : 'Read-only'}</span></p>
                    ${problem ? `<p class="sync-error" role="alert">${esc(problem)}</p>` : ''}
                    ${syncManagement(folder.githubReview?.pending ? [{ label: 'Review pending changes', value: 'review', icon: 'warning' }] : [])}`,
                buttons: [
                    { label: 'Disconnect this device', value: 'disconnect', empty: true, left: true },
                    { label: 'Sync now', value: 'now', primary: true },
                ],
            });
            if (action.value === 'close') return;
            try {
                if (action.value === 'settings') {
                    await githubConfiguration(id);
                    problem = '';
                } else if (action.value === 'now') {
                    ui.busy('Syncing…');
                    await flush();
                    const message = await syncGitHub(id);
                    if (ui.open && message) toast(message);
                    return;
                } else if (action.value === 'review') {
                    ui.close();
                    void reviewGitHubChanges(id);
                    return;
                } else if (action.value === 'disconnect') {
                    const answer = await ui.screen({
                        title: 'Disconnect this device?',
                        html: '<p>This folder and its documents stay on this device as independent copies. They will no longer send or receive changes. The GitHub repository and other devices are unchanged.</p>',
                        buttons: [
                            { label: 'Cancel', value: 'back' },
                            { label: 'Disconnect', value: 'confirm', primary: true },
                        ],
                    });
                    if (answer.value === 'confirm') {
                        ui.busy('Stopping sync…');
                        await disconnectGitHub(id);
                        toast('Sync is off. Your folder and documents are still here.');
                        return;
                    }
                }
            } catch (error) {
                problem = error instanceof Error ? error.message : 'Could not finish GitHub Sync.';
            }
        }
    } finally {
        ui.close();
    }
}

function githubConfiguration(id?: string): Promise<void> {
    const dialogId = id ? 'githubSettingsDialog' : 'githubImportDialog';
    let source = id ? node(id)?.github : undefined;
    if (document.getElementById(dialogId) || (id && !source)) return Promise.resolve();
    const title = id ? 'Sync settings' : 'Import from GitHub';
    const dialog = document.createElement('dialog');
    dialog.id = dialogId;
    dialog.className = 'su-dialog mars-modal sync-dialog';
    dialog.setAttribute('aria-labelledby', `${dialogId}Title`);
    dialog.innerHTML = `<header class="su-dialog-header"><h2 id="${dialogId}Title">${title}</h2><button type="button" class="su-dialog-close" aria-label="Close">${icon('cross')}</button></header>
        <form method="dialog" aria-labelledby="${dialogId}Title">
            <div class="su-dialog-body">
                ${source ? `<p><strong>${esc(source.repository)}</strong> · ${esc(source.branch)}</p>` : ''}
                <fieldset class="github-fields"><legend class="sr-only">GitHub connection</legend>
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
                    ${scheduleFields('githubTiming', true)}
                    ${source ? syncManagement([{ label: 'Manage exclusions', value: 'exclusions', icon: 'eyeSlash' }]) : ''}
                </fieldset>
                <p class="sync-error" role="alert" hidden></p>
            </div>
            <footer class="mars-modal-foot">${source ? '' : '<button type="button" class="btn btn-empty" value="cancel">Cancel</button>'}<button class="btn btn-primary" type="submit">${source ? 'Save' : 'Import folder'}</button></footer>
        </form>`;
    const controller = new AbortController();
    const byId = <T extends HTMLElement>(name: string) => dialog.querySelector<T>(`#${name}`)!;
    const picker = byId<HTMLSelectElement>('githubCredential');
    const manage = byId<HTMLButtonElement>('githubManageCredentials');
    const fields = bindScheduleFields(dialog, 'githubTiming', true);
    const problem = dialog.querySelector<HTMLElement>('.sync-error')!;
    const submit = dialog.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    const group = dialog.querySelector<HTMLFieldSetElement>('fieldset')!;
    const targetSource = () =>
        source ??
        githubSource(byId<HTMLInputElement>('githubRepo').value, byId<HTMLInputElement>('githubBranch').value);
    let labels: CredentialLabel[] = [];
    let selection = 'public';
    let chosen = false;
    let legacyAvailable = false;
    let loadingCredentials = true;
    const draft: Partial<SyncSchedule> =
        id && source ? readSchedule(githubScheduleKey(id), gitHubScheduleOptions(source)) : {};
    const owner = () => {
        try {
            return targetSource().repository.split('/')[0].toLowerCase();
        } catch {
            return '';
        }
    };
    const updateTiming = () => {
        const available = selection === 'legacy' ? legacyAvailable : labels.some((label) => label.id === selection);
        fields.render(githubSchedule(selection !== 'public', available), draft);
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
        for (const [name, list] of [
            [`Suggested for ${org}`, matches],
            ['Saved credentials', rest],
        ] as const) {
            if (!list.length) continue;
            const options = document.createElement('optgroup');
            options.label = name;
            for (const label of list)
                options.append(new Option(label.owner ? `${label.name} · ${label.owner}` : label.name, label.id));
            picker.append(options);
        }
        picker.value = selection;
        updateTiming();
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
            if (dialog.open) {
                problem.textContent = 'Could not read saved GitHub credentials.';
                problem.hidden = false;
            }
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
        updateTiming();
    });
    byId('githubTimingReceive').addEventListener('change', () => {
        draft.receive = fields.read().receive;
    });
    byId('githubTimingSend').addEventListener('change', () => {
        draft.send = fields.read().send;
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
    dialog.querySelector('#githubRepo')?.addEventListener('input', updatePicker);
    dialog.querySelector('#githubBranch')?.addEventListener('input', updatePicker);
    dialog
        .querySelectorAll('.su-dialog-close, [value="cancel"]')
        .forEach((button) => button.addEventListener('click', () => dialog.close()));
    if (source) submit.before(syncBackButton(() => dialog.close()));
    dialog.querySelector('[data-sync-action="exclusions"]')?.addEventListener('click', () => openGitHubExclusions(id!));
    dialog.querySelector('form')!.addEventListener('submit', async (event) => {
        event.preventDefault();
        if (submit.disabled) return;
        const schedule = fields.read();
        submit.disabled = group.disabled = true;
        problem.hidden = true;
        let download: SyncDialog | undefined;
        try {
            const legacy = selection === 'legacy';
            const named = !legacy && selection !== 'public';
            if (named && !labels.some((label) => label.id === selection))
                throw new Error('This credential is no longer available. Choose another credential.');
            const settings: GitHubSettings = {
                authorization: named
                    ? { credentialId: selection }
                    : legacy
                      ? { keepFolderToken: true }
                      : { publicOnly: true },
                schedule,
            };
            if (id) {
                await saveGitHubSettings(id, source?.authRevision, settings);
            } else {
                const target = targetSource();
                const current = (download = new SyncDialog('githubDownloadDialog'));
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
                    settings,
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
            }
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
            submit.disabled = group.disabled = false;
        }
    });
    return new Promise((resolve) => {
        dialog.addEventListener(
            'close',
            () => {
                controller.abort();
                hideTooltip();
                dialog.remove();
                resolve();
            },
            { once: true },
        );
        document.body.append(dialog);
        updatePicker();
        submit.disabled = true;
        hideTooltip();
        dialog.showModal();
        void loadCredentials();
    });
}
