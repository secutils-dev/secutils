// Review durable rename/delete intents without transmitting changes until they are approved.
import * as db from '../store/db.ts';
import { toast } from '../doc/viewer.js';
import { openSourceReview } from '../doc/review-workspace.ts';
import { esc, modal } from '../ui/ui.ts';
import { SyncDialog } from '../sync/dialog.ts';
import { reviseGitHubStructure } from './files.ts';
import type { StructuralJob } from './structure.ts';
import { readBlockedGitHubDelete } from './workspace.ts';

/** Open the repository's pending operations, or a specific rename from its sidebar badge. */
export async function reviewGitHubChanges(root: string, selected?: string) {
    if (document.getElementById('githubPendingDialog')) return;
    const ui = new SyncDialog('githubPendingDialog');
    const reviewed = new Map<string, string>();
    let issue = '';
    try {
        while (!ui.signal.aborted) {
            const jobs = (await db.getSide<StructuralJob[]>('github-jobs', root)) ?? [];
            const shown = selected ? jobs.filter((job) => job.id === selected) : jobs;
            if (!shown.length) {
                toast('No pending GitHub changes.');
                break;
            }
            const body = shown
                .map((job) => {
                    const label = job.kind === 'rename' ? 'Rename' : 'Delete';
                    const block = job.blocked;
                    const reason =
                        block?.reason === 'destination'
                            ? `${block.path} already exists on GitHub or blocks the destination.`
                            : block?.reason === 'renamed'
                              ? 'You and GitHub gave the same item different names. Choose which name to keep.'
                              : block?.reason === 'source'
                                ? `${block.path} changed on GitHub.`
                                : block?.reason === 'limit'
                                  ? 'This change is too large to send in one batch.'
                                  : block?.reason === 'excluded'
                                    ? `${block.path} is excluded from Sync. Include it to continue.`
                                    : block
                                      ? `This operation cannot safely change ${block.path}.`
                                      : 'Waiting to sync.';
                    if (block?.reason === 'renamed')
                        return `<section class="github-pending-item"><strong>Different names for the same item</strong><p>${esc(reason)}</p>
                    <dl class="github-rename-choices"><dt>Your name</dt><dd>${esc(job.to!)}</dd><dt>GitHub’s name</dt><dd>${esc(block.path)}</dd></dl>
                    <button type="button" class="btn btn-xs" data-sync-action="keep-upstream:${job.id}">Keep GitHub’s name</button> <button type="button" class="btn btn-primary btn-xs" data-sync-action="use-my-name:${job.id}">Use my name</button><p class="sync-fineprint">Keeping GitHub’s name changes only this copy. Using your name sends the rename to GitHub.</p></section>`;
                    return `<section class="github-pending-item"><strong>${label}${block ? ' blocked' : ' pending'}</strong><p>${esc(job.from)}${job.to ? ` → ${esc(job.to)}` : ''}</p><p>${esc(reason)}</p>
                ${job.kind === 'rename' && block?.reason === 'destination' ? `<button type="button" class="btn btn-xs" data-sync-action="rename:${job.id}">Choose another name</button>` : ''}
                    ${job.kind === 'delete' && block?.reason === 'source' && block.sha ? `<button type="button" class="btn btn-xs" data-sync-action="compare:${job.id}">Compare changes</button> <button type="button" class="btn btn-xs" data-sync-action="delete:${job.id}"${reviewed.get(job.id) === block.sha ? '' : ' disabled'}>Delete the latest version</button>` : ''}
                    <button type="button" class="btn btn-xs" data-sync-action="cancel:${job.id}">${job.kind === 'rename' ? 'Undo local rename' : job.folder ? 'Keep folder' : 'Keep file'}</button>${job.kind === 'rename' ? '<p class="sync-fineprint">This puts the original name back in MARS and does not change GitHub.</p>' : ''}</section>`;
                })
                .join('');
            const answer = await ui.screen({
                title: 'Review pending changes',
                html: `${body}${issue ? `<p class="sync-error" role="alert">${esc(issue)}</p>` : ''}`,
            });
            if (answer.value === 'close') break;
            const [action, id] = answer.value.split(':');
            const job = jobs.find((entry) => entry.id === id);
            if (!job) continue;
            issue = '';
            try {
                if (action === 'cancel') await reviseGitHubStructure(root, job, { kind: 'cancel' });
                if (action === 'keep-upstream') await reviseGitHubStructure(root, job, { kind: 'keep-upstream' });
                if (action === 'use-my-name') await reviseGitHubStructure(root, job, { kind: 'use-my-name' });
                if (action === 'rename') {
                    const result = await modal({
                        title: 'Choose another name',
                        field: { label: 'Name', value: '' },
                        buttons: [
                            { label: 'Cancel', value: '' },
                            { label: 'Rename', value: 'rename', primary: true },
                        ],
                        signal: ui.signal,
                    });
                    if (result.value) await reviseGitHubStructure(root, job, { kind: 'rename', name: result.text });
                }
                if (action === 'compare') {
                    const pair = await readBlockedGitHubDelete(root, job);
                    if (pair && ui.open) {
                        reviewed.set(job.id, pair.sha);
                        openSourceReview(pair.path, 'Local copy', pair.local, pair.remote, 'GitHub');
                    }
                }
                if (action === 'delete' && job.blocked?.reason === 'source' && job.blocked.sha) {
                    const block = job.blocked;
                    if (reviewed.get(job.id) !== block.sha) continue;
                    const result = await modal({
                        title: 'Delete the latest version?',
                        body: 'The version you just reviewed will be deleted from the GitHub branch. If GitHub changes it again, MARS will stop and ask you to review it again.',
                        buttons: [
                            { label: 'Cancel', value: '' },
                            { label: 'Delete the latest version', value: 'delete', danger: true },
                        ],
                        signal: ui.signal,
                    });
                    if (result.value)
                        await reviseGitHubStructure(root, job, {
                            kind: 'delete-latest',
                            path: block.path,
                            sha: block.sha!,
                        });
                }
            } catch (error) {
                issue = error instanceof Error ? error.message : 'Could not review this change.';
            }
        }
    } catch {
        toast('Could not open the pending GitHub changes.');
    } finally {
        ui.close();
    }
}
