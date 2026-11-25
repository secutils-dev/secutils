// Explicit GitHub operations publish against a fresh, complete tree, never a filtered or stale view.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import * as viewer from '../doc/viewer.js';
import { openThreads } from '../share/merge.ts';
import { ancestors, descendants, newId, nextOrder, textBytes, type TreeNode } from '../tree/model.ts';
import { writablePath } from './api.ts';
import { githubRoot } from './binding.ts';
import { excludedJobPath, githubExclusion } from './exclusions.ts';
import { knownGitHubPaths, scheduleGitHubSync, withGitHubJobLock } from './workspace.ts';
import type { StructuralJob } from './structure.ts';

type FileAction =
    | { kind: 'create' | 'duplicate'; name: string; text: string; parent: string; originalPath?: string }
    | { kind: 'rename'; name: string; id: string }
    | { kind: 'delete'; id: string };

const excludedChange = () => new Error('This change touches an excluded path. Include it in Sync before continuing.');

function pathSegment(value: string) {
    const name = value.trim();
    if (!name || /^[.]+$/.test(name) || name.toLowerCase() === '.git' || /[/\\\x00-\x1f\x7f]/.test(name))
        throw new Error('Use a file or folder name without slashes or special path characters.');
    return name;
}

const join = (parent: string, name: string) => (parent ? `${parent}/${name}` : name);
const folderPath = (nodes: TreeNode[], folder: TreeNode, root: string) =>
    folder.id === root
        ? ''
        : [...ancestors(nodes, folder.id), folder]
              .filter((n) => n.id !== root)
              .map((n) => pathSegment(n.name))
              .join('/');
const extension = (path: string) => path.match(/\.(?:md|markdown|mdx|txt)$/i)?.[0] ?? '.md';

/** Save local rename/delete intents without waiting for a network request. */
export async function queueGitHubStructure(root: string, action: Extract<FileAction, { id: string }>) {
    await viewer.flush();
    const binding = app.node(action.id)?.githubFile;
    if (binding?.root === root && !binding.sha) {
        // A file that has never been sent can move or disappear without changing the branch.
        const handled = await withGitHubJobLock(root, () =>
            app.serializePersistence(async () => {
                const current = (await db.readGitHubWorkingCopies([action.id])).get(action.id);
                const file = current?.node.githubFile;
                const repo = await db.getNode(root);
                if (!current || file?.root !== root || file.sha || current.conflict || !repo?.github?.write)
                    return false;
                const wasOpen = app.state.currentId === action.id;
                if (action.kind === 'delete') {
                    await db.putNodes(
                        [],
                        {},
                        { expected: [current], expectedGitHubRoot: root, removeGitHubNodes: [action.id] },
                    );
                } else {
                    const name = pathSegment(action.name);
                    if (name === current.node.name) return true;
                    const path = [...file.path.split('/').slice(0, -1), name + extension(file.path)].join('/');
                    if (githubExclusion(repo.githubExcludes)(path)) throw excludedChange();
                    if ((await knownGitHubPaths(root)).includes(path))
                        throw new Error(`That GitHub path is already in use: ${path}`);
                    await db.putNodes(
                        [{ ...current.node, name, githubFile: { ...file, path } }],
                        {},
                        { expected: [current], expectedGitHubRoot: root },
                    );
                }
                await app.load();
                if (wasOpen && action.kind === 'delete') app.closeDoc();
                else if (action.kind === 'rename' && app.state.currentId === action.id)
                    viewer.renameDocument(action.name.trim());
                return true;
            }),
        );
        if (handled) {
            scheduleGitHubSync(root);
            return;
        }
    }
    await app.serializePersistence(async () => {
        const open = app.state.currentId;
        await db.updateGitHubJobs(root, (jobs, nodes) => {
            const selected = nodes.find((n) => n.id === action.id);
            const repo = githubRoot(nodes, selected);
            if (!selected || selected.id === root || repo?.id !== root || !repo.github?.write)
                throw new Error('This item is no longer writable on GitHub.');
            const items = [selected, ...descendants(nodes, selected.id)];
            const ids = items.map((n) => n.id);
            if (items.some((n) => n.githubChange) || ancestors(nodes, selected.id).some((n) => n.githubChange))
                throw new Error('Finish the pending GitHub change before changing this item again.');
            if (items.length > 100) throw new Error('This folder has too many items to change at once.');
            const folder = selected.kind === 'folder';
            const from = folder ? folderPath(nodes, selected, root) : selected.githubFile?.path;
            if (!from) throw new Error('This file is not synced with GitHub.');
            const name = action.kind === 'rename' ? pathSegment(action.name) : selected.name;
            if (action.kind === 'rename' && name === selected.name) return { value: undefined, changed: [] };
            if (
                action.kind === 'rename' &&
                nodes.some(
                    (n) =>
                        n.id !== selected.id &&
                        n.parentId === selected.parentId &&
                        n.name === name &&
                        n.githubChange?.kind !== 'delete',
                )
            )
                throw new Error('That name already exists in this folder.');
            const to =
                action.kind === 'rename'
                    ? join(from.split('/').slice(0, -1).join('/'), name + (folder ? '' : extension(from)))
                    : undefined;
            const files = items
                .filter((n) => n.kind === 'doc')
                .map((n) => {
                    const file = n.githubFile;
                    if (
                        !file ||
                        file.root !== root ||
                        !writablePath(file.path) ||
                        file.conflict ||
                        (folder && !file.path.startsWith(`${from}/`))
                    )
                        throw new Error('Resolve conflicts and unsupported files before changing this item.');
                    return {
                        id: n.id,
                        from: file.path,
                        sha: file.sha,
                        ...(to ? { to: folder ? `${to}${file.path.slice(from.length)}` : to } : {}),
                    };
                });
            const id = newId(),
                job: StructuralJob = {
                    id,
                    node: selected.id,
                    kind: action.kind,
                    name: selected.name,
                    ids,
                    from,
                    to,
                    folder,
                    files,
                };
            if (excludedJobPath(job, githubExclusion(repo.githubExcludes))) throw excludedChange();
            jobs.push(job);
            selected.name = name;
            for (const item of items) item.githubChange = { id, kind: job.kind };
            return { value: undefined, changed: items };
        });
        await app.load();
        if (open && !app.node(open)) app.closeDoc();
        else if (open === action.id && action.kind === 'rename' && app.node(open))
            viewer.renameDocument(app.node(open)!.name);
    });
    scheduleGitHubSync(root);
}

/** Explicit changes to newer upstream paths require review. Cancel restores the original local item. */
export async function reviseGitHubStructure(
    root: string,
    expected: StructuralJob,
    resolution:
        | { kind: 'cancel' }
        | { kind: 'keep-upstream' }
        | { kind: 'use-my-name' }
        | { kind: 'rename'; name: string }
        | { kind: 'delete-latest'; path: string; sha: string },
) {
    await viewer.flush();
    await withGitHubJobLock(root, () =>
        app.serializePersistence(async () => {
            await db.updateGitHubJobs(root, (jobs, nodes) => {
                const job = jobs.find((item) => item.id === expected.id);
                if (!job || JSON.stringify(job) !== JSON.stringify(expected))
                    throw new Error('This change moved on. Reopen Sync to review the latest version.');
                if (job.kind === 'rename' && job.files.some((file) => file.attemptedSha) && !job.blocked)
                    throw new Error('Sync first to check whether GitHub accepted the rename.');
                const items = nodes.filter((n) => n.githubChange?.id === job.id);
                const selected = items.find((n) => n.id === job.node);
                if (!selected) throw new Error('The local item no longer exists.');
                const excludes = githubExclusion(nodes.find((n) => n.id === root)?.githubExcludes);
                if (resolution.kind === 'keep-upstream' || resolution.kind === 'use-my-name') {
                    if (job.kind !== 'rename' || job.blocked?.reason !== 'renamed')
                        throw new Error('Sync again to check where GitHub moved this item.');
                    const upstream = job.blocked.path;
                    const keep = resolution.kind === 'keep-upstream' || upstream === job.to;
                    // Earlier versions imported the upstream path before reporting the conflict.
                    // Reconcile those duplicates in the same transaction without losing local edits.
                    const duplicates = nodes.filter(
                        (n) =>
                            !n.githubChange &&
                            n.githubFile?.root === root &&
                            job.files.some(
                                (file) => n.githubFile?.path === `${upstream}${file.from.slice(job.from.length)}`,
                            ),
                    );
                    const duplicateFolders = job.folder
                        ? [
                              ...new Map(
                                  duplicates
                                      .flatMap((node) => ancestors(nodes, node.id))
                                      .filter((n) => n.id !== root && !n.githubChange)
                                      .map((n) => [n.id, n]),
                              ).values(),
                          ]
                        : [];
                    for (const file of job.files) {
                        const path = `${upstream}${file.from.slice(job.from.length)}`;
                        delete file.attemptedSha;
                        file.from = path;
                        const original = items.find((item) => item.id === file.id);
                        if (original?.githubFile) original.githubFile = { ...original.githubFile, path };
                    }
                    job.from = upstream;
                    if (excludedJobPath(job, excludes)) throw excludedChange();
                    if (keep) {
                        jobs.splice(jobs.indexOf(job), 1);
                        const name = upstream.split('/').at(-1)!;
                        selected.name = job.folder ? name : name.slice(0, -extension(name).length);
                        for (const item of items) delete item.githubChange;
                    } else {
                        delete job.blocked;
                        for (const item of items) item.githubChange = { id: job.id, kind: 'rename' };
                    }
                    const candidates = duplicateFolders
                        .filter((folder) => {
                            const path = folderPath(nodes, folder, root);
                            return path === upstream || path.startsWith(`${upstream}/`);
                        })
                        .sort((a, b) => folderPath(nodes, a, root).length - folderPath(nodes, b, root).length);
                    return {
                        value: undefined,
                        changed: items,
                        dropDuplicateGitHubNodes: duplicates.map((node) => node.id),
                        pruneGitHubFolders: candidates.map((node) => node.id),
                        clearIndex: true,
                    };
                } else if (resolution.kind === 'cancel') {
                    jobs.splice(jobs.indexOf(job), 1);
                    selected.name = job.name;
                    for (const item of items) delete item.githubChange;
                } else {
                    if (resolution.kind === 'rename') {
                        if (job.kind !== 'rename') throw new Error('This item is not waiting to be renamed.');
                        const name = pathSegment(resolution.name);
                        if (
                            nodes.some(
                                (n) =>
                                    n.id !== selected.id &&
                                    n.parentId === selected.parentId &&
                                    n.name === name &&
                                    n.githubChange?.kind !== 'delete',
                            )
                        )
                            throw new Error('That name already exists in this folder.');
                        const to = join(
                            job.from.split('/').slice(0, -1).join('/'),
                            name + (job.folder ? '' : extension(job.from)),
                        );
                        if (to === job.from) throw new Error('Choose a different name, or cancel the rename.');
                        job.to = to;
                        selected.name = name;
                        for (const file of job.files) {
                            file.to = job.folder ? `${to}${file.from.slice(job.from.length)}` : to;
                            delete file.attemptedSha;
                        }
                    } else {
                        if (
                            job.kind !== 'delete' ||
                            job.blocked?.reason !== 'source' ||
                            job.blocked.path !== resolution.path ||
                            job.blocked.sha !== resolution.sha
                        )
                            throw new Error('Review the latest version before approving deletion.');
                        job.files.find((file) => file.from === resolution.path)!.sha = resolution.sha;
                    }
                    if (excludedJobPath(job, excludes)) throw excludedChange();
                    delete job.blocked;
                    for (const item of items) item.githubChange = { id: job.id, kind: job.kind };
                }
                return { value: undefined, changed: items };
            });
            await app.load();
            if (app.state.currentId === expected.node && app.current()) viewer.renameDocument(app.current()!.name);
        }),
    );
    scheduleGitHubSync(root);
}

/** Save a file addition in the working copy. A scheduled or manual send publishes it. */
export async function changeGitHubFiles(root: string, action: FileAction) {
    if ('id' in action) return queueGitHubStructure(root, action);
    await viewer.flush();
    const id = await withGitHubJobLock(root, () =>
        app.serializePersistence(async () => {
            const nodes = await db.allNodes();
            const selected = nodes.find((n) => n.id === action.parent);
            const repo = githubRoot(nodes, selected);
            if (!selected || selected.githubChange || repo?.id !== root || !repo.github?.write)
                throw new Error('This item is no longer writable on GitHub.');
            if (selected.kind !== 'folder') throw new Error('Choose a GitHub folder.');
            const prefix = folderPath(nodes, selected, root),
                name = pathSegment(action.name),
                path = join(prefix, name + extension(action.originalPath ?? ''));
            if (githubExclusion(repo.githubExcludes)(path)) throw excludedChange();
            if ((await knownGitHubPaths(root)).includes(path))
                throw new Error(`That GitHub path is already in use: ${path}`);
            const id = newId(),
                now = Date.now();
            const node: TreeNode = {
                id,
                originId: id,
                name,
                kind: 'doc',
                parentId: selected.id,
                order: nextOrder(nodes, selected.id),
                createdAt: now,
                updatedAt: now,
                threads: openThreads(action.text),
                size: textBytes(action.text),
                // An empty SHA is a local addition. Only a verified upstream acknowledgement sets its baseline.
                githubFile: { root, path, sha: '' },
            };
            await db.putNodes([node], { [id]: action.text }, { expectedGitHubRoot: root, allowGitHubInserts: true });
            await app.load();
            return id;
        }),
    );
    scheduleGitHubSync(root);
    return id;
}
