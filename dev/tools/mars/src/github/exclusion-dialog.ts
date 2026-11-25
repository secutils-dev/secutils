// Exclusions change this browser's checkout only. Saving a rule does not send a GitHub request.
import * as app from '../app.ts';
import * as viewer from '../doc/viewer.js';
import * as db from '../store/db.ts';
import { syncBackButton } from '../sync/dialog.ts';
import { ancestors, type TreeNode } from '../tree/model.ts';
import { dialogCloseButton } from '../ui/ui.ts';
import { githubRoot } from './binding.ts';
import {
    excludedJobPath,
    githubExclusion,
    literalExclusion,
    MAX_EXCLUSION_LENGTH,
    visibleGitHubNodes,
} from './exclusions.ts';
import { knownGitHubPaths, withGitHubJobLock } from './workspace.ts';

type RuleChange = (nodes: TreeNode[], root: TreeNode) => string;

async function saveRules(root: string, change: RuleChange) {
    await withGitHubJobLock(root, async () => {
        await viewer.flush();
        await app.serializePersistence(async () => {
            const open = app.state.currentId;
            await db.updateGitHubExclusions(root, (nodes, folder, jobs) => {
                const rules = change(nodes, folder).replace(/\r\n/g, '\n').replace(/\n+$/, '');
                if (rules.length > MAX_EXCLUSION_LENGTH)
                    throw new Error('The exclusion rules are too long. Try a broader pattern.');
                const excludes = githubExclusion(rules);
                if (jobs.some((job) => excludedJobPath(job, excludes)))
                    throw new Error(
                        'These rules match a pending rename or deletion. Review pending changes before excluding it.',
                    );
                if (open && !viewer.canApplyConnection()) {
                    const projected = nodes.map((node) =>
                        node.id === root ? { ...folder, githubExcludes: rules } : node,
                    );
                    if (!visibleGitHubNodes(projected).some((node) => node.id === open))
                        throw new Error('Finish the open edit before excluding this file.');
                }
                return rules;
            });
            await app.load();
            if (open && !app.node(open)) app.closeDoc();
        });
    });
}

/** Add one exact rule from the context menu. Keep cached text and local edits in the library. */
export async function excludeGitHubItem(root: string, id: string) {
    try {
        let before = '',
            after = '';
        await saveRules(root, (nodes, folder) => {
            const item = nodes.find((node) => node.id === id);
            if (!item || item.github || githubRoot(nodes, item)?.id !== root)
                throw new Error('This item is no longer synced with GitHub.');
            const isFolder = item.kind === 'folder';
            const path = isFolder
                ? [...ancestors(nodes, id).filter((node) => node.id !== root), item].map((node) => node.name).join('/')
                : item.githubFile?.path;
            if (!path || /[\r\n]/.test(path)) throw new Error('This item does not have an excludable GitHub path.');
            before = folder.githubExcludes ?? '';
            if (githubExclusion(before)(path + (isFolder ? '/' : ''))) return before;
            after = [before, literalExclusion(path, isFolder)].filter(Boolean).join('\n');
            return after;
        });
        viewer.toast(
            'Excluded from Sync on this device.',
            !after
                ? undefined
                : {
                      label: 'Undo',
                      run: () => {
                          void saveRules(root, (_nodes, current) => {
                              if (current.githubExcludes !== after)
                                  throw new Error('The rules changed. Open Manage exclusions to restore this item.');
                              return before;
                          }).catch((error: Error) => viewer.toast(error.message));
                      },
                  },
        );
    } catch (error) {
        viewer.toast(error instanceof Error ? error.message : 'Could not exclude this item.');
    }
}

/** Edit repository-relative rules and preview them against the paths already known to this device. */
export function openGitHubExclusions(root: string) {
    const folder = app.node(root);
    if (!folder?.github || document.getElementById('githubExclusionDialog')) return;
    const original = folder.githubExcludes ?? '';
    const dialog = document.createElement('dialog');
    dialog.id = 'githubExclusionDialog';
    dialog.className = 'su-dialog mars-modal sync-dialog';
    dialog.setAttribute('aria-labelledby', 'githubExclusionTitle');
    dialog.innerHTML = `<header class="su-dialog-header"><h2 id="githubExclusionTitle">Manage exclusions</h2></header>
        <form method="dialog">
            <div class="su-dialog-body github-exclusions">
                <p>Excluded items stay on GitHub. MARS hides them and skips them during Sync. Local edits stay in this browser.</p>
                <label class="field-label" for="githubExclusionRules">Exclusion rules</label>
                <textarea class="textarea" id="githubExclusionRules" name="rules" rows="6" maxlength="${MAX_EXCLUSION_LENGTH}" aria-describedby="githubExclusionHelp" autocapitalize="none" spellcheck="false" autofocus></textarea>
                <p id="githubExclusionHelp" class="sync-fineprint">Use one <a href="https://git-scm.com/docs/gitignore#_pattern_format" target="_blank" rel="noopener noreferrer">.gitignore pattern</a> per line: <code>/README.md</code> for an exact path, <code>/drafts/</code> for a folder, or <code>**/*.txt</code> for a wildcard. Rules are case-sensitive and do not follow renames.</p>
                <p class="sync-fineprint">To restore an item, remove its matching rule. New files appear on the next sync.</p>
                <div><button class="btn" type="button" data-preview>Preview matches</button></div>
                <div role="status" aria-live="polite" data-preview-result hidden></div>
                <p class="sync-error" role="alert" hidden></p>
            </div>
            <footer class="mars-modal-foot"><button class="btn btn-primary" type="submit">Save exclusions</button></footer>
        </form>`;
    const form = dialog.querySelector('form')!;
    const field = form.querySelector('textarea')!;
    const submit = form.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    const preview = form.querySelector<HTMLButtonElement>('[data-preview]')!;
    const result = form.querySelector<HTMLElement>('[data-preview-result]')!;
    const problem = form.querySelector<HTMLElement>('.sync-error')!;
    field.value = original;
    let revision = 0;
    field.addEventListener('input', () => {
        revision++;
        result.hidden = problem.hidden = true;
    });
    preview.addEventListener('click', async () => {
        const version = revision;
        preview.disabled = true;
        try {
            const paths = await knownGitHubPaths(root);
            if (!dialog.open || version !== revision) return;
            const matches = paths.filter(githubExclusion(field.value));
            const count = document.createElement('p');
            count.textContent = matches.length
                ? `${matches.length} known ${matches.length === 1 ? 'file matches' : 'files match'}.`
                : 'No known files match these rules.';
            result.replaceChildren(count);
            if (matches.length) {
                const list = document.createElement('ul');
                const shown = matches.slice(0, 6);
                if (shown.length < matches.length) shown.push(`… and ${matches.length - shown.length} more`);
                for (const path of shown) {
                    const item = document.createElement('li');
                    item.textContent = path;
                    list.append(item);
                }
                result.append(list);
            }
            result.hidden = false;
        } catch {
            problem.textContent = 'Could not read the list of files from this device.';
            problem.hidden = false;
        } finally {
            preview.disabled = false;
        }
    });
    form.addEventListener('submit', async (event) => {
        event.preventDefault();
        if (submit.disabled) return;
        const rules = field.value;
        submit.disabled = field.readOnly = true;
        problem.hidden = true;
        try {
            await saveRules(root, (_nodes, current) => {
                if ((current.githubExcludes ?? '') !== original)
                    throw new Error(
                        'The rules changed in another tab. Reopen Manage exclusions to see the latest rules.',
                    );
                return rules;
            });
            dialog.close();
            viewer.toast('Exclusions saved on this device.');
        } catch (error) {
            problem.textContent = error instanceof Error ? error.message : 'Could not save the exclusion rules.';
            problem.hidden = false;
        } finally {
            submit.disabled = field.readOnly = false;
        }
    });
    dialog.querySelector('header')!.append(dialogCloseButton(() => dialog.close()));
    submit.before(syncBackButton(() => dialog.close()));
    dialog.addEventListener('close', () => dialog.remove(), { once: true });
    document.body.append(dialog);
    dialog.showModal();
}
