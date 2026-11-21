// A local working copy of a GitHub branch. Pulls compare remote blob identity with saved local text.
// This preview never turns local deletions, moves, or unsupported files into remote writes.
import * as app from '../app.ts';
import * as viewer from '../doc/viewer.js';
import { readFileSource } from '../io/source.ts';
import { openThreads } from '../share/merge.ts';
import * as db from '../store/db.ts';
import { newId, textBytes, type TreeNode } from '../tree/model.ts';
import { gitBlobOid, GitHubApi, GitHubError, writablePath, type GitHubFile, type GitHubSource } from './api.ts';
import { githubEnabled, githubRoot } from './binding.ts';
import { fileDecision } from './merge.ts';

const DEBOUNCE_MS = 6_000;
const POLL_MS = 60_000;
const PUBLIC_POLL_MS = 180_000;
const MAX_PULL_BYTES = 20 * 1024 * 1024;
const MAX_BATCH_FILES = 30;
const MAX_BATCH_TEXT_BYTES = 4 * 1024 * 1024;

interface Session {
    token: string;
    head?: string;
    files?: Map<string, GitHubFile>;
    busy?: Promise<string>;
    timer?: ReturnType<typeof setTimeout>;
    retryAt?: number;
    nextPoll?: number;
    message?: string;
}

// PATs live only in memory. The browser library holds paths, text, and blob IDs, not PATs.
const sessions = new Map<string, Session>();

/** Sync status or the next action for a GitHub folder. */
export const githubStatus = (id: string) => sessions.get(id)?.message ?? 'Reconnect GitHub in this tab to resume sync.';

/** Start a repository connection. New folders are imported only after all requested files have been read. */
export async function connectGitHub(
    source: GitHubSource,
    token: string,
    write: boolean,
    id?: string,
    signal?: AbortSignal,
) {
    if (!githubEnabled() || db.isTemporaryStorage()) throw new Error('Open the dev library to connect GitHub.');
    const api = new GitHubApi(token);
    const target = await api.resolve(source, signal);
    const existing = id
        ? app.node(id)
        : app.state.nodes.find(
              (node) =>
                  node.github?.repository.toLowerCase() === target.repository.toLowerCase() &&
                  node.github.branch === target.branch,
          );
    if (id && !existing?.github) throw new Error('This GitHub folder no longer exists.');
    const rootId = existing?.id ?? newId();
    if (existing && sessions.get(existing.id)?.busy) throw new Error('Wait for the current GitHub sync to finish.');
    const session = { token, nextPoll: Date.now() + (token ? POLL_MS : PUBLIC_POLL_MS) } as Session;
    const root: TreeNode = existing
        ? { ...existing }
        : {
              id: rootId,
              parentId: null,
              kind: 'folder',
              name: app.uniqueName(target.repository.split('/')[1], null),
              order: app.state.nodes.length,
              createdAt: Date.now(),
              updatedAt: Date.now(),
              open: true,
          };
    root.github = { ...target, write: Boolean(token) && write };
    const message = await runGitHub(root, session, { signal, pullOnly: true });
    sessions.set(root.id, session);
    return message;
}

/** Pull and flush one workspace, or join a run already in progress. */
export function syncGitHub(id: string): Promise<string> {
    const root = app.node(id),
        session = sessions.get(id);
    if (!root?.github || !session) return Promise.reject(new Error('Reconnect GitHub in this tab to resume sync.'));
    if (session.busy) return session.busy;
    if ((session.retryAt ?? 0) > Date.now()) return Promise.resolve(githubStatus(id));
    session.busy = runGitHub(root, session)
        .catch((error) => {
            session.message = error instanceof Error ? error.message : 'Could not sync with GitHub.';
            session.retryAt = Date.now() + Math.max(POLL_MS, error instanceof GitHubError ? error.retryAfterMs : 0);
            return session.message;
        })
        .finally(() => {
            session.busy = undefined;
        });
    return session.busy;
}

/** After the editor saves an attached file, schedule one coalesced commit for the branch. */
export function githubSaved(node: TreeNode) {
    if (!node.githubFile) return;
    const id = node.githubFile.root,
        session = sessions.get(id);
    if (!session?.token) return;
    clearTimeout(session.timer);
    session.timer = setTimeout(() => {
        void syncGitHub(id).catch(() => {});
    }, DEBOUNCE_MS);
}

/** Poll only connected workspaces in a visible, online tab. Public and private sessions use the same budget. */
export function installGitHubPolling() {
    if (!githubEnabled()) return;
    document.querySelector<HTMLElement>('[data-github-preview-only]')?.removeAttribute('hidden');
    const poll = () => {
        if (document.hidden || !navigator.onLine || db.isTemporaryStorage()) return;
        for (const [id, session] of sessions) {
            if (!app.node(id)?.github) {
                clearTimeout(session.timer);
                sessions.delete(id);
            } else if ((session.nextPoll ?? 0) <= Date.now()) {
                session.nextPoll = Date.now() + (session.token ? POLL_MS : PUBLIC_POLL_MS);
                void syncGitHub(id).catch(() => {});
            }
        }
    };
    setInterval(poll, POLL_MS);
    document.addEventListener('visibilitychange', poll);
    window.addEventListener('online', poll);
}

/** True when the viewer must not edit a GitHub attachment (HTML remains a read-only import). */
export function githubReadOnly(node?: TreeNode) {
    return (
        !!node?.githubFile && (!githubRoot(app.state.nodes, node)?.github?.write || !writablePath(node.githubFile.path))
    );
}

async function runGitHub(root: TreeNode, session: Session, options: { signal?: AbortSignal; pullOnly?: boolean } = {}) {
    if (!root.github || db.isTemporaryStorage()) throw new Error('GitHub requires a browser library.');
    const source = root.github,
        api = new GitHubApi(session.token),
        signal = options.signal;
    const head = await api.head(source, signal);
    const files =
        session.head === head.commit && session.files
            ? session.files
            : new Map((await api.files(source, head.tree, signal)).map((file) => [file.path, file]));
    const library = await db.allNodes();
    const previousRoot = library.find((n) => n.id === root.id);
    const docs = new Map(library.filter((n) => n.githubFile?.root === root.id).map((n) => [n.githubFile!.path, n]));
    const folders = new Map<string, TreeNode>([['', previousRoot ?? root]]);
    for (const file of docs.values()) {
        let parent = file.parentId;
        const path = file.githubFile!.path.split('/');
        while (parent && parent !== root.id && path.length > 1) {
            path.pop();
            const folder = library.find((n) => n.id === parent);
            if (!folder) break;
            folders.set(path.join('/'), folder);
            parent = folder.parentId;
        }
    }
    const updated: TreeNode[] = options.pullOnly ? [previousRoot ? { ...previousRoot, github: source } : root] : [];
    const texts: Record<string, string> = {},
        extras: Record<string, unknown> = {};
    const expected: db.IncomingSnapshot[] = [];
    const edits: { id: string; path: string; text: string; previousSha: string }[] = [];
    const fetchSize = [...files.values()]
        .filter((file) => docs.get(file.path)?.githubFile?.sha !== file.sha)
        .reduce((sum, file) => sum + file.size, 0);
    if (fetchSize > MAX_PULL_BYTES) throw new Error('The changed files exceed the GitHub preview download budget.');
    const folderFor = (parts: string[]): string => {
        let parent = root.id,
            prefix = '';
        for (const name of parts) {
            prefix = prefix ? `${prefix}/${name}` : name;
            let folder = folders.get(prefix);
            if (!folder) {
                folder = {
                    id: newId(),
                    kind: 'folder',
                    parentId: parent,
                    name,
                    order: folders.size,
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                    open: true,
                };
                folders.set(prefix, folder);
                updated.push(folder);
            }
            parent = folder.id;
        }
        return parent;
    };
    let conflicts = 0,
        editedBytes = 0;
    for (const file of files.values()) {
        signal?.throwIfAborted();
        const n = docs.get(file.path);
        const current = n ? await db.getDocument(n.id) : undefined;
        if (n && (!current || current.node.githubFile?.sha !== n.githubFile?.sha)) continue;
        const base = n ? await db.getSide('github-base', n.id) : undefined;
        const raw = current && n?.githubFile?.sha === file.sha ? base : await api.text(source, file, signal);
        if (raw == null) continue;
        const parts = file.path.split('/');
        const filename = parts.pop()!;
        const incoming = writablePath(file.path) ? raw : readFileSource(raw, filename).text;
        if (!current) {
            const now = Date.now(),
                id = newId();
            updated.push({
                id,
                originId: id,
                kind: 'doc',
                parentId: folderFor(parts),
                name: readFileSource('', filename).name,
                order: updated.length,
                createdAt: now,
                updatedAt: now,
                threads: openThreads(incoming),
                size: textBytes(incoming),
                githubFile: { root: root.id, path: file.path, sha: file.sha },
            });
            texts[id] = incoming;
            extras[`github-base:${id}`] = incoming;
            continue;
        }
        const id = current.node.id;
        const decision = fileDecision(base, current.text, incoming);
        if (decision === 'conflict') {
            conflicts++;
            continue;
        }
        if (decision === 'push') {
            const bytes = textBytes(current.text);
            if (
                !options.pullOnly &&
                source.write &&
                session.token &&
                writablePath(file.path) &&
                edits.length < MAX_BATCH_FILES &&
                editedBytes + bytes <= MAX_BATCH_TEXT_BYTES
            ) {
                edits.push({ id, path: file.path, text: current.text, previousSha: file.sha });
                editedBytes += bytes;
            }
            continue;
        }
        if (current.node.githubFile?.sha === file.sha) continue;
        if (id === app.state.currentId && (viewer.isEditing() || !viewer.canPublishConnection())) {
            conflicts++;
            continue;
        }
        expected.push({ ...current, opened: await db.getSide('opened', id) });
        updated.push({
            ...current.node,
            githubFile: { root: root.id, path: file.path, sha: file.sha },
            ...(decision === 'pull'
                ? { updatedAt: Date.now(), size: textBytes(incoming), threads: openThreads(incoming) }
                : {}),
        });
        if (decision === 'pull') texts[id] = incoming;
        extras[`github-base:${id}`] = incoming;
    }
    const removed = [...docs.keys()].filter((path) => !files.has(path)).length;
    signal?.throwIfAborted();
    await app.serializePersistence(async () => {
        if (db.isTemporaryStorage() || (previousRoot && !app.node(root.id)))
            throw new Error('The GitHub folder is no longer available.');
        if (updated.length) {
            if (
                app.state.currentId &&
                Object.hasOwn(texts, app.state.currentId) &&
                (viewer.isEditing() || !viewer.canPublishConnection())
            )
                throw new Error('Finish the open edit and sync again. Your changes are still here.');
            await db.putNodes(updated, texts, { expected, extras });
            await app.load();
            await app.adoptRemoteChange(updated.map((n) => n.id));
        }
    });
    if (edits.length) {
        if (!app.node(root.id)?.github) throw new Error('The GitHub folder is no longer available.');
        await api.commit(source, head.commit, edits, signal);
        // Keep the exact acknowledged bytes, not newer editor text saved during the upload.
        const oids = await Promise.all(edits.map((edit) => gitBlobOid(edit.text)));
        await app.serializePersistence(async () => {
            for (const [i, edit] of edits.entries())
                await db.acknowledgeGitHubBlob(edit.id, edit.previousSha, oids[i], edit.text);
            await app.load();
        });
        // Re-read the branch next time. A lost response is also safe: identical remote text is an ACK.
        session.head = undefined;
    } else {
        session.head = head.commit;
        session.files = files;
    }
    session.message = [
        edits.length ? `Committed ${edits.length} file${edits.length === 1 ? '' : 's'} to GitHub.` : 'Checked GitHub.',
        conflicts ? `${conflicts} conflicting or open edits were kept locally.` : '',
        removed ? `${removed} missing remote files were kept locally.` : '',
    ]
        .filter(Boolean)
        .join(' ');
    return session.message;
}
