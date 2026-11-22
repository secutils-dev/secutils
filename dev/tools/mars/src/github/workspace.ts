// A local working copy of a GitHub branch. Pulls compare remote blob identity with saved local text.
// Pending renames and deletions publish only after complete-tree checks; unsupported files stay read-only.
import * as app from '../app.ts';
import * as viewer from '../doc/viewer.js';
import { readFileSource } from '../io/source.ts';
import { openThreads } from '../share/merge.ts';
import * as db from '../store/db.ts';
import { newId, textBytes, type TreeNode } from '../tree/model.ts';
import {
    gitBlobOid,
    GitHubApi,
    GitHubError,
    writablePath,
    type GitHubFile,
    type GitHubHead,
    type GitHubSource,
    type GitHubTreeEntry,
} from './api.ts';
import { githubRoot } from './binding.ts';
import { excludedJobPath, githubExclusion } from './exclusions.ts';
import { fileDecision, mergeGitHubText, type GitHubConflict } from './merge.ts';
import { openGitHubToken, type GitHubCredential } from './credentials.ts';
import {
    hasGitHubCredential,
    saveGitHubCredential,
    withGitHubCredential,
    type GitHubAuthorization,
} from './credential-store.ts';
import { findGitHubRenames, similarGitHubRenames } from './reconcile.ts';
import { LOCAL_STRUCTURE_CHANGED, planGitHubStructure, type StructuralJob } from './structure.ts';

const DEBOUNCE_MS = 6_000;
const POLL_MS = 60_000;
const PUBLIC_POLL_MS = 180_000;
const MAX_PULL_BYTES = 20 * 1024 * 1024;
const MAX_BATCH_FILES = 30;
const MAX_BATCH_TEXT_BYTES = 4 * 1024 * 1024;

interface Session {
    // Named credentials never keep plaintext in a session; each operation reads the current record.
    token: string;
    attachment?: string;
    scope?: string;
    busy?: Promise<string>;
    timer?: ReturnType<typeof setTimeout>;
    retryAt?: number;
    nextPoll?: number;
    message?: string;
    uploadNotice?: string;
    paused?: boolean;
}

interface BranchIndex {
    v: 1;
    source: GitHubSource;
    authenticated: boolean;
    authScope?: string;
    head: GitHubHead;
    files: GitHubFile[];
    checkedAt: number;
}

interface PollState {
    retryAt: number;
    message: string;
    authScope?: string;
}

const getIndex = async (root: string, source: GitHubSource, auth: GitHubAuthorization) => {
    const index = await db.getSide<BranchIndex>('github-index', root);
    return index?.v === 1 &&
        index.source.repository === source.repository &&
        index.source.branch === source.branch &&
        index.authenticated === Boolean(auth.token) &&
        index.authScope === auth.scope
        ? index
        : undefined;
};

/** Paths from the last GitHub check, including excluded files. Does not contact GitHub. */
export async function knownGitHubPaths(root: string) {
    const [index, nodes] = await Promise.all([db.getSide<BranchIndex>('github-index', root), db.allNodes()]);
    return [
        ...new Set([
            ...(index?.files.map((file) => file.path) ?? []),
            ...nodes.flatMap((node) => (node.githubFile?.root === root ? [node.githubFile.path] : [])),
        ]),
    ];
}

const pollInterval = (token: string) => (token ? POLL_MS : PUBLIC_POLL_MS);

// Serialize same-folder passes across tabs. Automatic passes don't queue behind another tab's work.
async function withBranch<T>(id: string, automatic: boolean, run: () => Promise<T>): Promise<T | undefined> {
    if (!navigator.locks) return run();
    return navigator.locks.request(`mars.github:${id}`, { ifAvailable: automatic }, (lock) =>
        lock ? run() : undefined,
    );
}

/** Serialize an explicit GitHub file operation with pulls, uploads, and disconnect. */
export async function withGitHubWrite<T>(id: string, run: (api: GitHubApi, source: GitHubSource) => Promise<T>) {
    if (disconnecting.has(id)) throw new Error('This folder is disconnecting.');
    return withBranch(id, false, async () => {
        const source = (await db.getNode(id))?.github;
        if (disconnecting.has(id) || !source?.write)
            throw new Error('Connect a write-capable token in Sync to change GitHub files.');
        return authorize(id, source, sessions.get(id), async ({ token }) => {
            if (!token) throw new Error('Connect a write-capable token in Sync to change GitHub files.');
            return run(new GitHubApi(token), source);
        });
    });
}

/** Wait for any in-flight write before resolving a job, even when its token is no longer available. */
export const withGitHubJobLock = <T>(id: string, run: () => Promise<T>) => withBranch(id, false, run);

// Active PATs are held in memory. The library may also hold a sealed credential in a private side record.
const sessions = new Map<string, Session>();
const disconnecting = new Set<string>();

/** Import progress. A zero total means GitHub is still returning branch details. */
export type GitHubProgress = (done: number, total: number, stage: 'details' | 'download' | 'save') => void;

/** Sync status or the next action for a GitHub folder. */
const githubStatus = (id: string) =>
    sessions.get(id)?.message ??
    (!sessions.has(id) && (app.node(id)?.github?.authenticated ?? app.node(id)?.github?.write)
        ? 'Enter a token to resume sync.'
        : '');

/** True when Sync can use the current configuration. Named credentials reload on each operation. */
export const hasGitHubSession = (id: string) => {
    const source = app.node(id)?.github,
        session = sessions.get(id);
    return !!source && (!!source.credentialId || (!!session && session.attachment === source.authRevision));
};

/** Reuse a tab-only token when its settings change, without filling it into the form. */
const activeGitHubToken = (id: string) => {
    const source = app.node(id)?.github,
        session = sessions.get(id);
    return !source?.credentialId && session?.attachment === source?.authRevision ? (session?.token ?? '') : '';
};

/** Check for a folder-only token without decrypting it or returning its value to the UI. */
export async function hasFolderGitHubToken(id: string) {
    return !!(activeGitHubToken(id) || (await db.getSide('github-auth', id)));
}

function authorize<T>(
    id: string,
    source: NonNullable<TreeNode['github']>,
    session: Session | undefined,
    run: (auth: GitHubAuthorization) => Promise<T>,
) {
    if (source.credentialId) return withGitHubCredential(source.credentialId, run);
    return (async () => {
        if (!(source.authenticated ?? source.write)) return run({ token: '' });
        const token = (session?.attachment === source.authRevision && session?.token) || (await savedGitHubToken(id));
        if (!token) throw new Error('Enter a token in Sync to resume this GitHub connection.');
        return run({ token, scope: source.authRevision });
    })();
}

/** Stop polling and drop the in-memory token when deleting the local folder. */
export function forgetGitHubSession(id: string) {
    clearTimeout(sessions.get(id)?.timer);
    sessions.delete(id);
}

/** Stop this browser's GitHub Sync. Keep the folder and its files at their existing locations. */
export async function disconnectGitHub(id: string) {
    if (disconnecting.has(id)) return;
    disconnecting.add(id);
    try {
        await viewer.flush();
        await withBranch(id, false, () =>
            app.serializePersistence(async () => {
                await db.detachGitHubFolder(id);
                forgetGitHubSession(id);
                await app.load();
                viewer.refreshConnection();
            }),
        );
    } finally {
        disconnecting.delete(id);
    }
}

/** Create a local document with a new identity, leaving the read-only GitHub file connected. */
export async function makeGitHubCopy(id: string) {
    await viewer.flush();
    const original = await db.getDocument(id);
    if (!original || !githubReadOnly(original.node)) return;
    const copy = await app.createDoc({
        name: app.uniqueName(`${original.node.name} (copy)`, null),
        parentId: null,
        text: original.text,
        readingWidth: original.node.readingWidth,
    });
    if (app.state.currentId === copy.id) viewer.setViewMode('edit');
}

/** Start a repository connection. New folders are imported only after all requested files have been read. */
export async function connectGitHub(
    source: GitHubSource,
    authorization: { credentialId: string } | { keepFolderToken: true } | { publicOnly: true },
    write: boolean,
    id?: string,
    signal?: AbortSignal,
    progress?: GitHubProgress,
) {
    if (db.isTemporaryStorage()) throw new Error('Open your library to connect GitHub.');
    progress?.(0, 0, 'details');
    const credentialId = 'credentialId' in authorization ? authorization.credentialId : undefined;
    const authRevision = newId();
    const explicitFolder = id ? app.node(id) : undefined;
    if (id && !explicitFolder?.github) throw new Error('This GitHub folder no longer exists.');
    const reuse = 'keepFolderToken' in authorization;
    const use = <T>(run: (auth: GitHubAuthorization) => Promise<T>) =>
        credentialId
            ? withGitHubCredential(credentialId, run)
            : reuse && id && explicitFolder?.github && !explicitFolder.github.credentialId
              ? authorize(id, explicitFolder.github, sessions.get(id), run)
              : run({ token: '' });
    const { target, scope } = await use(async ({ token, scope }) => ({
        target: await new GitHubApi(token).resolve(source, signal),
        scope,
    }));
    const existing = id
        ? explicitFolder
        : app.state.nodes.find(
              (node) =>
                  node.github?.repository.toLowerCase() === target.repository.toLowerCase() &&
                  node.github.branch === target.branch,
          );
    const rootId = existing?.id ?? newId();
    if (existing && sessions.get(existing.id)?.busy) throw new Error('Wait for the current GitHub sync to finish.');
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
    return withBranch(root.id, false, () =>
        use(async (auth) => {
            if (scope !== auth.scope) throw new Error('This GitHub credential changed. Try again.');
            if (existing?.github) {
                const current = (await db.getNode(root.id))?.github;
                if (!current || current.authRevision !== existing.github.authRevision)
                    throw new Error('This GitHub connection changed in another tab. Reopen Sync and try again.');
            }
            root.github = {
                ...target,
                write: Boolean(auth.token) && write,
                authenticated: Boolean(auth.token),
                credentialId,
                authRevision,
            };
            const session: Session = {
                token: credentialId ? '' : auth.token,
                attachment: authRevision,
                scope: reuse ? authRevision : scope,
                nextPoll: Date.now() + pollInterval(auth.token),
            };
            const message = await runGitHub(root, session, reuse ? { ...auth, scope: authRevision } : auth, {
                signal,
                pullOnly: true,
                clearFolderToken: !reuse,
                progress,
                newAttachment: !existing,
            });
            const previous = sessions.get(root.id);
            clearTimeout(previous?.timer);
            sessions.set(root.id, session);
            return message;
        }),
    );
}

/** Retrieve a saved token for reconnect without putting it back into the form. */
async function savedGitHubToken(id: string) {
    const auth = await db.getSide<GitHubCredential | null>('github-auth', id);
    return auth ? openGitHubToken(auth, id) : '';
}

/** Save a folder-only PAT under a name and bind this folder to it, without contacting GitHub. */
export async function reuseGitHubToken(id: string, label: { name: string; owner: string }) {
    const credentialId = await withBranch(id, false, async () => {
        const root = await db.getNode(id);
        if (!root?.github || root.github.credentialId) throw new Error('This folder no longer uses its own token.');
        const token = activeGitHubToken(id) || (await savedGitHubToken(id));
        if (!token) throw new Error('This folder has no token to save.');
        const credentialId = await saveGitHubCredential(label, token);
        return app.serializePersistence(async () => {
            const current = await db.getNode(id);
            if (!current?.github) throw new Error('This folder is no longer synced with GitHub.');
            const authRevision = newId();
            await db.putNodes(
                [{ ...current, github: { ...current.github, credentialId, authRevision } }],
                {},
                {
                    expectedGitHubRoot: id,
                    replaceGitHubConnection: true,
                    extras: { [`github-auth:${id}`]: null },
                },
            );
            forgetGitHubSession(id);
            sessions.set(id, { token: '', attachment: authRevision, nextPoll: Date.now() + POLL_MS });
            await app.load();
            return credentialId;
        });
    });
    return credentialId!;
}

/** Forget stale sessions when another tab changes a folder's authorization. */
export async function refreshGitHubSessions(ids: string[]) {
    for (const id of ids) {
        const source = app.node(id)?.github,
            session = sessions.get(id);
        if (session && (!source || session.attachment !== source.authRevision)) forgetGitHubSession(id);
        if (source && !sessions.has(id)) {
            await restoreGitHubSession(id, source);
            const restored = sessions.get(id);
            if (restored) restored.nextPoll = Date.now() + pollInterval(restored.token || (source.credentialId ?? ''));
        }
    }
}

/** Refresh status and retries after a reusable credential changes, without making a request. */
export async function refreshGitHubCredential(id: string) {
    const found = await hasGitHubCredential(id);
    for (const root of app.state.nodes) {
        if (root.github?.credentialId !== id) continue;
        const session = sessions.get(root.id);
        if (!session) continue;
        session.retryAt = undefined;
        session.paused = !found;
        session.message = found ? undefined : 'GitHub Sync is paused. Choose another credential in Sync.';
        if (!found) clearTimeout(session.timer);
    }
}

/** Resume saved and public connections after reading the browser library. */
export async function restoreGitHubConnections() {
    if (db.isTemporaryStorage()) return;
    for (const node of app.state.nodes) {
        if (node.github && !sessions.has(node.id)) await restoreGitHubSession(node.id, node.github);
    }
    if (document.hidden || !navigator.onLine) return;
    for (const [id, session] of sessions) {
        if (session.nextPoll || session.paused) continue;
        session.nextPoll = Date.now() + pollInterval(session.token || (app.node(id)?.github?.credentialId ?? ''));
        await syncGitHub(id, true).catch(() => {});
    }
}

async function restoreGitHubSession(id: string, source: NonNullable<TreeNode['github']>) {
    try {
        const credentialId = source.credentialId;
        const token = credentialId ? '' : await savedGitHubToken(id);
        if (!credentialId && !token && (source.authenticated ?? source.write)) return;
        const paused = credentialId ? !(await hasGitHubCredential(credentialId)) : false;
        if (!sessions.has(id) && app.node(id)?.github?.authRevision === source.authRevision && app.node(id)?.github)
            sessions.set(id, {
                token,
                attachment: source.authRevision,
                paused,
                message: paused ? 'GitHub Sync is paused. Choose another credential in Sync.' : undefined,
            });
    } catch {
        // Local documents remain readable. Reconnecting replaces an unavailable credential.
    }
}

/** Pull and flush one workspace, or join a run already in progress. */
export async function syncGitHub(id: string, automatic = false, reportError = false): Promise<string> {
    const source = app.node(id)?.github;
    let session = sessions.get(id);
    if (disconnecting.has(id)) return '';
    if (!session && source?.credentialId) {
        session = { token: '', attachment: source.authRevision };
        sessions.set(id, session);
    }
    if (!source || !session) return Promise.reject(new Error('Reconnect GitHub in this tab to resume sync.'));
    if (session.busy) {
        const status = await session.busy;
        if (reportError && (session.retryAt ?? 0) > Date.now()) throw new Error(status);
        return status;
    }
    session.busy = withBranch(id, automatic, async () => {
        const root = await db.getNode(id);
        if (disconnecting.has(id) || !root?.github) return '';
        const current = session;
        return authorize(id, root.github, current, async (auth) => {
            if (current.scope !== auth.scope) {
                current.scope = auth.scope;
                current.retryAt = undefined;
                current.message = undefined;
            }
            if ((current.retryAt ?? 0) > Date.now()) return current.message ?? '';
            const key = `github-poll:${id}:${auth.token ? 'auth' : 'public'}`;
            const backoff = await db.getMeta<PollState>(key);
            if (backoff && backoff.authScope === auth.scope && backoff.retryAt > Date.now()) {
                current.retryAt = backoff.retryAt;
                return (current.message = backoff.message);
            }
            try {
                return await runGitHub(root, current, auth, { automatic });
            } catch (error) {
                if (error instanceof GitHubError && error.rateLimited) {
                    await db.setMeta(key, {
                        retryAt: Date.now() + Math.max(POLL_MS, error.retryAfterMs),
                        message: error.message,
                        authScope: auth.scope,
                    });
                }
                throw error;
            }
        });
    })
        .catch((error) => {
            if (error instanceof Error && error.message === LOCAL_STRUCTURE_CHANGED) {
                scheduleGitHubSync(id);
                return '';
            }
            session.message = error instanceof Error ? error.message : 'Could not sync with GitHub.';
            session.retryAt =
                error instanceof GitHubError && error.status === 401
                    ? Infinity
                    : Date.now() + Math.max(POLL_MS, error instanceof GitHubError ? error.retryAfterMs : 0);
            return session.message;
        })
        .then((message) => message ?? githubStatus(id))
        .finally(() => {
            session.busy = undefined;
        });
    const status = await session.busy;
    if (reportError && (session.retryAt ?? 0) > Date.now()) throw new Error(status);
    return status;
}

/** After the editor saves an attached file, schedule one coalesced commit for the branch. */
export function githubSaved(node: TreeNode) {
    if (!node.githubFile) return;
    scheduleGitHubSync(node.githubFile.root);
}

/** Schedule queued changes in the same debounce window as document edits. */
export function scheduleGitHubSync(id: string) {
    const session = sessions.get(id);
    const source = app.node(id)?.github;
    if (
        disconnecting.has(id) ||
        !session ||
        session.paused ||
        (!session.token && !source?.credentialId) ||
        !source?.write
    )
        return;
    clearTimeout(session.timer);
    session.timer = setTimeout(() => {
        if (sessions.get(id) !== session) return;
        void syncGitHub(id, false, true).then(
            () => {
                session.uploadNotice = undefined;
            },
            (error: unknown) => {
                if (sessions.get(id) !== session) return;
                const message = error instanceof Error ? error.message : 'Could not sync with GitHub.';
                if (session.uploadNotice !== message) {
                    session.uploadNotice = message;
                    viewer.toast(`Your edit is saved in MARS but not synced to GitHub. ${message}`);
                }
            },
        );
    }, DEBOUNCE_MS);
}

/** Compare a retained local deletion with the exact upstream version that blocked it. */
export async function readBlockedGitHubDelete(root: string, job: StructuralJob) {
    if (job.kind !== 'delete' || job.blocked?.reason !== 'source' || !job.blocked.sha)
        throw new Error('This change has no upstream text to compare.');
    const block = job.blocked;
    const id = job.files.find((file) => file.from === block.path)?.id;
    return withGitHubWrite(root, async (api, source) => {
        const head = await api.head(source);
        const file = (await api.files(source, head.tree!)).find(
            (file) => file.path === block.path && file.sha === block.sha,
        );
        if (!id || !file) throw new Error('GitHub changed again. Sync and review the new version.');
        const remote = await api.text(source, file, undefined, head.commit);
        if (remote === null || (await gitBlobOid(remote)) !== file.sha)
            throw new Error('This upstream file cannot be reviewed in MARS.');
        return { local: (await db.getText(id)) ?? '', remote, sha: file.sha, path: block.path };
    });
}

async function publishStructuralJobs(
    root: string,
    api: GitHubApi,
    source: GitHubSource,
    head: GitHubHead,
    jobs: StructuralJob[],
    excludes: ReturnType<typeof githubExclusion>,
    signal?: AbortSignal,
) {
    const tree = new Map((await api.tree(source, head.tree!, signal)).map((entry) => [entry.path, entry]));
    const finished: StructuralJob[] = [],
        ready: StructuralJob[] = [],
        blocked = new Map<string, StructuralJob['blocked']>();
    const changes: { path: string; text: string }[] = [],
        deletions: string[] = [];
    let batchBytes = 0;
    for (const job of jobs) {
        const excluded = excludedJobPath(job, excludes);
        if (excluded) {
            blocked.set(job.id, { reason: 'excluded', path: excluded });
            continue;
        }
        const plan = planGitHubStructure(job, tree);
        if ('blocked' in plan) {
            blocked.set(job.id, plan.blocked);
            continue;
        }
        if (plan.complete) {
            finished.push(job);
            continue;
        }
        const local = await db.readGitHubWorkingCopies(job.files.map((file) => file.id));
        const files = await Promise.all(
            job.files.map(async (file) => {
                const text = local.get(file.id)?.text ?? '';
                return { ...file, local: text, ...(file.to ? { attemptedSha: await gitBlobOid(text) } : {}) };
            }),
        );
        const bytes = files.reduce((size, file) => size + (file.to ? textBytes(file.local) : 0), 0);
        if (bytes > MAX_BATCH_TEXT_BYTES || files.some((file) => file.to && textBytes(file.local) > 2 * 1024 * 1024)) {
            blocked.set(job.id, { reason: 'limit', path: job.from });
            continue;
        }
        if (batchBytes + bytes > MAX_BATCH_TEXT_BYTES || changes.length + deletions.length + files.length * 2 > 200)
            continue;
        batchBytes += bytes;
        for (const change of plan.changes) {
            const file = files.find((entry) => entry.id === change.id)!;
            deletions.push(file.from);
            if (file.to) changes.push({ path: file.to, text: file.local });
        }
        // Reserve paths for the rest of this batch. Another operation must not overwrite them.
        for (const file of files)
            if (file.to) tree.set(file.to, { path: file.to, sha: file.attemptedSha!, type: 'blob', mode: '100644' });
        ready.push({ ...job, files, blocked: undefined });
    }
    const attempted = new Map(ready.map((job) => [job.id, job]));
    // Save exact attempted content before the network write so a lost acknowledgement is recoverable.
    if (attempted.size || blocked.size)
        await app.serializePersistence(async () => {
            await db.updateGitHubJobs(root, (stored, nodes) => {
                const changed: TreeNode[] = [];
                for (const [i, job] of stored.entries()) {
                    const next = attempted.get(job.id),
                        block = blocked.get(job.id);
                    if (next) stored[i] = next;
                    else if (block) job.blocked = block;
                    else continue;
                    for (const node of nodes.filter((n) => n.githubChange?.id === job.id)) {
                        node.githubChange = { id: job.id, kind: job.kind, ...(block ? { blocked: true } : {}) };
                        changed.push(node);
                    }
                }
                return { value: undefined, changed };
            });
            await app.load();
        });
    let published: string | undefined;
    if (ready.length && !disconnecting.has(root)) {
        if (changes.length || deletions.length) {
            published = await api.commit(source, head.commit, changes, signal, deletions);
        }
        finished.push(...ready);
    }
    if (finished.length)
        await app.serializePersistence(async () => {
            const current = await db.readGitHubWorkingCopies(
                finished.flatMap((job) => job.files.map((file) => file.id)),
            );
            await db.updateGitHubJobs(root, (stored, nodes) => {
                const changed: TreeNode[] = [],
                    remove: string[] = [];
                const bases: Record<string, string> = {};
                for (const job of finished) {
                    const index = stored.findIndex((entry) => entry.id === job.id);
                    if (index < 0) continue;
                    stored.splice(index, 1);
                    for (const node of nodes.filter((n) => n.githubChange?.id === job.id)) {
                        delete node.githubChange;
                        const file = job.files.find((entry) => entry.id === node.id);
                        if (job.kind === 'delete') {
                            if (!file || (file.local !== undefined && current.get(file.id)?.text === file.local))
                                remove.push(node.id);
                            else {
                                delete node.githubFile;
                                node.parentId = null;
                                node.name += ' (local edits)';
                                changed.push(node);
                            }
                        } else {
                            if (file?.to) {
                                node.githubFile = { root, path: file.to, sha: file.attemptedSha ?? file.sha };
                                if (file.attemptedSha) bases[node.id] = file.local ?? '';
                            }
                            changed.push(node);
                        }
                    }
                }
                return { value: undefined, changed, remove, bases, clearIndex: true };
            });
            await app.load();
        });
    return published;
}

/** Poll only connected workspaces in a visible, online tab. Public and private sessions use the same budget. */
export function installGitHubPolling() {
    const poll = () => {
        if (document.hidden || !navigator.onLine || db.isTemporaryStorage()) return;
        for (const [id, session] of sessions) {
            if (!app.node(id)?.github) {
                clearTimeout(session.timer);
                sessions.delete(id);
            } else if (!session.paused && (session.nextPoll ?? 0) <= Date.now()) {
                session.nextPoll =
                    Date.now() + pollInterval(session.token || (app.node(id)?.github?.credentialId ?? ''));
                void syncGitHub(id, true).catch(() => {});
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

/** Apply an explicit resolution only if the local text and saved remote conflict still match. */
export async function resolveGitHubConflict(id: string, expected: GitHubConflict, text: string) {
    await viewer.flush();
    let resolved: TreeNode | undefined;
    const root = app.node(id)?.githubFile?.root;
    if (!root) throw new Error('This GitHub document is no longer connected.');
    await withBranch(root, false, () =>
        app.serializePersistence(async () => {
            const excludes = githubExclusion((await db.getNode(root))?.githubExcludes);
            const current = (await db.readGitHubWorkingCopies([id])).get(id);
            const binding = current?.node.githubFile,
                conflict = current?.conflict;
            if (
                !current ||
                !binding ||
                excludes(binding.path) ||
                !conflict ||
                conflict.sha !== expected.sha ||
                conflict.remote !== expected.remote ||
                conflict.base !== expected.base ||
                current.text !== expected.local ||
                githubReadOnly(current.node)
            )
                throw new Error('This conflict changed. Reopen it to review the latest versions.');
            if (app.state.currentId === id && (viewer.isEditing() || !viewer.canPublishConnection()))
                throw new Error('Finish the open edit before applying this resolution.');
            resolved = {
                ...current.node,
                githubFile: { root, path: binding.path, sha: conflict.sha },
                updatedAt: Date.now(),
                size: textBytes(text),
                threads: openThreads(text),
            };
            await db.putNodes(
                [resolved],
                { [id]: text },
                {
                    expected: [current],
                    extras: {
                        [`github-base:${id}`]: conflict.remote,
                        [`github-conflict:${id}`]: null,
                    },
                },
            );
            await app.load();
            await app.adoptRemoteChange([id]);
        }),
    );
    if (resolved) githubSaved(resolved);
}

async function runGitHub(
    root: TreeNode,
    session: Session,
    auth: GitHubAuthorization,
    options: {
        signal?: AbortSignal;
        pullOnly?: boolean;
        clearFolderToken?: boolean;
        progress?: GitHubProgress;
        automatic?: boolean;
        newAttachment?: boolean;
    } = {},
) {
    if (!root.github || db.isTemporaryStorage()) throw new Error('GitHub requires a browser library.');
    const storedRoot = options.newAttachment ? undefined : await db.getNode(root.id);
    if (!options.newAttachment && !storedRoot?.github) throw new Error('This folder is no longer synced with GitHub.');
    const rules = storedRoot?.githubExcludes ?? '';
    const excludes = githubExclusion(rules);
    const source = root.github,
        api = new GitHubApi(auth.token, undefined, options.automatic ? 'low' : 'auto'),
        signal = options.signal;
    const cached = await getIndex(root.id, source, auth);
    const jobs = (await db.getSide<StructuralJob[]>('github-jobs', root.id)) ?? [];
    if (
        options.automatic &&
        !jobs.some((job) => !job.blocked) &&
        cached &&
        Date.now() - cached.checkedAt < pollInterval(auth.token)
    )
        return session.message ?? '';
    // A different token may yield a different representation. Reconnect always does a fresh branch check.
    let head = await api.head(source, signal, options.pullOnly ? undefined : cached?.head);
    let reusable = cached;
    const published =
        !options.pullOnly && source.write && auth.token && jobs.length && head.tree
            ? await publishStructuralJobs(root.id, api, source, head, jobs, excludes, signal)
            : undefined;
    if (published) {
        const previous = head.commit;
        head = await api.head(source, signal);
        // The branch read can briefly lag behind the accepted commit. Do not reimport obsolete paths.
        if (head.commit === previous) return '';
        reusable = undefined;
    }
    const remaining = (await db.getSide<StructuralJob[]>('github-jobs', root.id)) ?? [];
    const reserved = (path: string) =>
        remaining.some(
            (job) =>
                path === job.from ||
                path === job.to ||
                (job.blocked?.reason === 'renamed' &&
                    (path === job.blocked.path || (job.folder && path.startsWith(`${job.blocked.path}/`)))) ||
                (job.folder && (path.startsWith(`${job.from}/`) || (job.to && path.startsWith(`${job.to}/`)))),
        );
    const files =
        reusable && (reusable.head.commit === head.commit || (head.tree && reusable.head.tree === head.tree))
            ? reusable.files
            : await api.files(source, head.tree!, signal);
    const index: BranchIndex = {
        v: 1,
        source: { repository: source.repository, branch: source.branch },
        authenticated: Boolean(auth.token),
        authScope: auth.scope,
        head,
        files,
        checkedAt: Date.now(),
    };
    const library = await db.allNodes();
    const previousRoot = library.find((n) => n.id === root.id);
    const docs = new Map(library.filter((n) => n.githubFile?.root === root.id).map((n) => [n.githubFile!.path, n]));
    const remotePaths = new Set(files.map((file) => file.path));
    const activeOld = [...docs]
        .filter(([path, node]) => !reserved(path) && !node.githubChange && !excludes(path))
        .map(([path, node]) => ({ path, sha: node.githubFile!.sha }));
    const activeNew = files.filter((file) => !reserved(file.path) && !excludes(file.path));
    const snapshots = await db.readGitHubWorkingCopies(activeOld.map(({ path }) => docs.get(path)!.id));
    const renames = findGitHubRenames(activeOld, activeNew);
    const exactSources = new Set(renames.values());
    if (
        cached &&
        cached.head.commit !== head.commit &&
        activeOld.some((file) => !remotePaths.has(file.path) && !exactSources.has(file.path)) &&
        activeNew.some((file) => !docs.has(file.path) && !renames.has(file.path))
    ) {
        for (const [to, from] of findGitHubRenames(
            activeOld,
            activeNew,
            await api.renameHints(source, cached.head.commit, head.commit, signal),
        ))
            renames.set(to, from);
    }
    const oldFor = (path: string) => docs.get(renames.get(path) ?? path);
    const folders = new Map<string, TreeNode>([['', previousRoot ?? root]]);
    const localById = new Map(library.map((node) => [node.id, node]));
    // Git does not store empty directories. Reuse locally created folders when the first file appears.
    for (const node of library) {
        if (node.kind !== 'folder' || node.id === root.id) continue;
        const parts = [node.name];
        let parent = node.parentId;
        while (parent && parent !== root.id) {
            const folder = localById.get(parent);
            if (!folder || parts.length >= 256) break;
            parts.unshift(folder.name);
            parent = folder.parentId;
        }
        if (parent === root.id) folders.set(parts.join('/'), node);
    }
    const importedFolders = new Set<string>();
    for (const file of docs.values()) {
        let parent = file.parentId;
        const path = file.githubFile!.path.split('/');
        while (parent && parent !== root.id && path.length > 1) {
            path.pop();
            const folder = localById.get(parent);
            if (!folder) break;
            importedFolders.add(folder.id);
            folders.set(path.join('/'), folder);
            parent = folder.parentId;
        }
    }
    const updated: TreeNode[] = options.pullOnly ? [previousRoot ? { ...previousRoot, github: source } : root] : [];
    const texts: Record<string, string> = {},
        extras: Record<string, unknown> = {};
    const expected: db.IncomingSnapshot[] = [];
    const edits: { id: string; path: string; text: string; previousSha: string }[] = [];
    if (options.clearFolderToken) extras[`github-auth:${root.id}`] = null;
    const changedFiles = activeNew.filter((file) => {
        const existing = oldFor(file.path);
        return (
            existing?.githubFile?.sha !== file.sha &&
            (!existing || snapshots.get(existing.id)?.conflict?.sha !== file.sha)
        );
    });
    const fetchSize = changedFiles.reduce((sum, file) => sum + file.size, 0);
    if (fetchSize > MAX_PULL_BYTES) throw new Error('The changed files exceed the GitHub Sync download budget.');
    const downloaded = new Map<string, string | null>();
    const blobs = new Map<string, Promise<string | null>>();
    let completed = 0;
    options.progress?.(0, changedFiles.length, changedFiles.length ? 'download' : 'save');
    // Private repositories cannot use raw content or archives. Group small blobs into a bounded GraphQL read.
    if (!source.public && auth.token) {
        const unique = [...new Map(changedFiles.map((file) => [file.sha, file])).values()];
        let batch: GitHubFile[] = [],
            bytes = 0;
        const flush = async () => {
            if (batch.length < 2) return;
            try {
                for (const [sha, text] of await api.textBatch(source, batch, signal))
                    blobs.set(sha, Promise.resolve(text));
            } catch (error) {
                if (!(error instanceof GitHubError) || error.rateLimited || ![400, 403, 422].includes(error.status))
                    throw error;
                // This credential cannot use the GraphQL query. Use authenticated REST for the remaining files.
                return false;
            }
        };
        let supported = true;
        for (const file of unique) {
            if (file.size > 1024 * 1024) continue;
            if (batch.length === 24 || bytes + file.size > 1024 * 1024) {
                if ((await flush()) === false) {
                    supported = false;
                    break;
                }
                batch = [];
                bytes = 0;
            }
            batch.push(file);
            bytes += file.size;
        }
        if (supported) await flush();
    }
    // Use a small, bounded batch for public CDN reads. Authenticated blob API calls stay sequential.
    const parallel = source.public ? 4 : 1;
    for (let start = 0; start < changedFiles.length; start += parallel) {
        await Promise.all(
            changedFiles.slice(start, start + parallel).map(async (file) => {
                let blob = blobs.get(file.sha);
                if (!blob) {
                    blob = api.text(source, file, signal, head.commit);
                    blobs.set(file.sha, blob);
                }
                downloaded.set(file.path, await blob);
                options.progress?.(++completed, changedFiles.length, 'download');
            }),
        );
    }
    const knownSources = new Set(renames.values());
    const oldText = new Map<string, string>(),
        newText = new Map<string, string>();
    for (const { path } of activeOld) {
        if (remotePaths.has(path) || knownSources.has(path)) continue;
        const base = snapshots.get(docs.get(path)!.id)?.base;
        if (base !== undefined && writablePath(path)) oldText.set(path, base);
    }
    for (const [path, text] of downloaded)
        if (text !== null && !docs.has(path) && !renames.has(path) && writablePath(path)) newText.set(path, text);
    for (const { from, to } of similarGitHubRenames(oldText, newText)) renames.set(to, from);
    // Reuse an imported folder only when all its tracked paths moved together under the same prefix.
    const renameBySource = new Map([...renames].map(([to, from]) => [from, to]));
    const movedFolders = new Map<string, TreeNode>();
    const prefixPaths = (paths: Iterable<string>) => {
        const result = new Map<string, string[]>();
        for (const path of paths) {
            for (let end = path.lastIndexOf('/'); end > 0; end = path.lastIndexOf('/', end - 1)) {
                const prefix = path.slice(0, end),
                    group = result.get(prefix);
                if (group) group.push(path);
                else result.set(prefix, [path]);
            }
        }
        return result;
    };
    const oldFolders = prefixPaths(docs.keys()),
        nextFolders = prefixPaths(remotePaths);
    for (const [prefix, folder] of folders) {
        if (!prefix || !importedFolders.has(folder.id) || folder.githubChange || nextFolders.has(prefix)) continue;
        const children = oldFolders.get(prefix) ?? [];
        const targets = children.map((path) => {
            const target = renameBySource.get(path),
                suffix = path.slice(prefix.length);
            return target?.endsWith(suffix) ? target.slice(0, -suffix.length) : undefined;
        });
        const target = targets[0];
        if (target && !folders.has(target) && !movedFolders.has(target) && targets.every((value) => value === target))
            movedFolders.set(target, folder);
    }
    // Relocate these folders before rendering the file moves. Do not leave their old path in the lookup.
    const relocatedFolderIds = new Set([...movedFolders.values()].map((folder) => folder.id));
    for (const [path, folder] of folders) if (relocatedFolderIds.has(folder.id)) folders.delete(path);
    const folderFor = (parts: string[]): string => {
        let parent = root.id,
            prefix = '';
        for (const name of parts) {
            prefix = prefix ? `${prefix}/${name}` : name;
            let folder = folders.get(prefix);
            if (!folder) {
                const moved = movedFolders.get(prefix);
                folder = moved
                    ? { ...moved, parentId: parent, name }
                    : {
                          id: newId(),
                          kind: 'folder',
                          parentId: parent,
                          name,
                          order: folders.size,
                          createdAt: Date.now(),
                          updatedAt: Date.now(),
                          open: true,
                      };
                if (moved) expected.push({ node: moved, text: '' });
                folders.set(prefix, folder);
                updated.push(folder);
            }
            parent = folder.id;
        }
        return parent;
    };
    let conflicts = 0,
        editedBytes = 0;
    let deadline = performance.now() + 50;
    for (const file of activeNew) {
        if (performance.now() >= deadline) {
            if (typeof globalThis.scheduler?.yield === 'function') await scheduler.yield();
            else await new Promise<void>((resolve) => setTimeout(resolve, 0));
            deadline = performance.now() + 50;
        }
        signal?.throwIfAborted();
        const n = oldFor(file.path);
        if (n?.githubChange) continue;
        const current = n ? snapshots.get(n.id) : undefined;
        if (n && (!current || current.node.githubFile?.sha !== n.githubFile?.sha)) continue;
        const base = current?.base;
        const raw =
            current && n?.githubFile?.sha === file.sha
                ? base
                : current?.conflict?.sha === file.sha
                  ? current.conflict.remote
                  : downloaded.get(file.path);
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
        const moved = renames.has(file.path);
        const placement = moved ? { parentId: folderFor(parts), name: readFileSource('', filename).name } : {};
        const decision = fileDecision(base, current.text, incoming);
        let nextText = decision === 'pull' ? incoming : current.text;
        if (decision === 'conflict') {
            if (
                current.conflict?.sha === file.sha &&
                current.conflict.local === current.text &&
                current.conflict.base === (base ?? '')
            ) {
                conflicts++;
                continue;
            }
            const parts =
                base === undefined || !writablePath(file.path) ? null : mergeGitHubText(base, current.text, incoming);
            if (!parts || parts.some((part) => !('text' in part))) {
                conflicts++;
                if (
                    moved ||
                    current.conflict?.sha !== file.sha ||
                    current.conflict.local !== current.text ||
                    current.conflict.base !== (base ?? '')
                ) {
                    expected.push(current);
                    updated.push({
                        ...current.node,
                        ...placement,
                        githubFile: { ...current.node.githubFile!, path: file.path, conflict: file.sha },
                    });
                    extras[`github-conflict:${id}`] = {
                        base: base ?? '',
                        local: current.text,
                        remote: incoming,
                        sha: file.sha,
                    } satisfies GitHubConflict;
                }
                continue;
            }
            nextText = parts.map((part) => ('text' in part ? part.text : '')).join('');
        }
        const changesBody = nextText !== current.text;
        if (changesBody && id === app.state.currentId && (viewer.isEditing() || !viewer.canPublishConnection())) {
            conflicts++;
            continue;
        }
        if (nextText !== incoming) {
            const bytes = textBytes(nextText);
            if (
                !options.pullOnly &&
                source.write &&
                auth.token &&
                writablePath(file.path) &&
                edits.length < MAX_BATCH_FILES &&
                editedBytes + bytes <= MAX_BATCH_TEXT_BYTES
            ) {
                edits.push({ id, path: file.path, text: nextText, previousSha: file.sha });
                editedBytes += bytes;
            }
        }
        if (!moved && current.node.githubFile?.sha === file.sha && !current.node.githubFile?.conflict && !changesBody)
            continue;
        expected.push(current);
        updated.push({
            ...current.node,
            ...placement,
            githubFile: { root: root.id, path: file.path, sha: file.sha },
            ...(changesBody
                ? {
                      updatedAt: Date.now(),
                      size: textBytes(nextText),
                      threads: openThreads(nextText),
                      pending: true as const,
                  }
                : {}),
        });
        if (changesBody) {
            texts[id] = nextText;
            if (!current.node.pending) extras[`base:${id}`] = current.text;
        }
        if (current.conflict) extras[`github-conflict:${id}`] = null;
        extras[`github-base:${id}`] = incoming;
    }
    const remove: string[] = [],
        recover: string[] = [];
    for (const { path } of activeOld) {
        if (remotePaths.has(path) || renameBySource.has(path)) continue;
        const node = docs.get(path)!;
        const current = snapshots.get(node.id);
        if (!current || (node.id === app.state.currentId && !viewer.canApplyConnection())) continue;
        expected.push(current);
        if (current.base !== undefined && current.base === current.text && !current.conflict) remove.push(node.id);
        else {
            const copy = { ...current.node, parentId: null, name: `${current.node.name} (local edits)` };
            delete copy.githubFile;
            updated.push(copy);
            recover.push(node.id);
        }
    }
    // Prune vacated imported folders only. Empty folders deliberately created in MARS stay local.
    const finalNodes = new Map(library.map((node) => [node.id, node]));
    for (const node of updated) finalNodes.set(node.id, node);
    for (const id of remove) finalNodes.delete(id);
    const childCounts = new Map<string, number>();
    for (const node of finalNodes.values())
        if (node.parentId) childCounts.set(node.parentId, (childCounts.get(node.parentId) ?? 0) + 1);
    const vacant = [...importedFolders].filter((id) => !childCounts.get(id));
    while (vacant.length) {
        const id = vacant.pop()!,
            folder = finalNodes.get(id);
        if (!folder || folder.githubChange || folder.github || !importedFolders.has(id)) continue;
        expected.push({ node: localById.get(id)!, text: '' });
        remove.push(id);
        finalNodes.delete(id);
        if (folder.parentId) {
            const count = (childCounts.get(folder.parentId) ?? 1) - 1;
            childCounts.set(folder.parentId, count);
            if (count === 0) vacant.push(folder.parentId);
        }
    }
    options.progress?.(completed, changedFiles.length, 'save');
    signal?.throwIfAborted();
    await app.serializePersistence(async () => {
        signal?.throwIfAborted();
        if (db.isTemporaryStorage() || (previousRoot && !app.node(root.id)))
            throw new Error('The GitHub folder is no longer available.');
        const open = app.state.currentId;
        if (updated.length || remove.length) {
            if (
                app.state.currentId &&
                (Object.hasOwn(texts, app.state.currentId) || remove.includes(app.state.currentId)) &&
                !viewer.canApplyConnection()
            )
                throw new Error('Finish the open edit and sync again. Your changes are still here.');
        }
        extras[`github-index:${root.id}`] = index;
        await db.putNodes(updated, texts, {
            expected,
            extras,
            expectedGitHubRoot: options.newAttachment ? undefined : root.id,
            expectedGitHubJobs: remaining.map((job) => job.id),
            expectedGitHubExcludes: rules,
            allowGitHubInserts: true,
            replaceGitHubConnection: options.pullOnly,
            removeGitHubNodes: remove,
            recoverGitHubNodes: recover,
        });
        if (updated.length || remove.length) {
            await app.load();
            if (open && !app.node(open)) app.closeDoc();
            else {
                await app.adoptRemoteChange(updated.map((n) => n.id));
                viewer.refreshConnection();
            }
        }
    });
    if (edits.length) {
        if (disconnecting.has(root.id) || !(await db.getNode(root.id))?.github) return '';
        const commit = await api.commit(source, head.commit, edits, signal);
        // Keep the exact acknowledged bytes, not newer editor text saved during the upload.
        const oids = await Promise.all(edits.map((edit) => gitBlobOid(edit.text)));
        await app.serializePersistence(async () => {
            for (const [i, edit] of edits.entries())
                await db.acknowledgeGitHubBlob(edit.id, edit.previousSha, oids[i], edit.text);
            if (app.node(root.id)?.github) {
                const replacements = new Map(
                    edits.map((edit, i) => [edit.path, { path: edit.path, sha: oids[i], size: textBytes(edit.text) }]),
                );
                await db.putSide('github-index', root.id, {
                    ...index,
                    head: { commit, tree: null },
                    files: files.map((file) => replacements.get(file.path) ?? file),
                } satisfies BranchIndex);
            }
            await app.load();
        });
    }
    const pulled = Object.keys(texts).length;
    session.message = [
        edits.length ? `Synced ${edits.length} file${edits.length === 1 ? '' : 's'} to GitHub.` : '',
        pulled && !options.pullOnly ? `Updated ${pulled} file${pulled === 1 ? '' : 's'} from GitHub.` : '',
        conflicts ? `${conflicts} conflicting or open edits were kept locally.` : '',
        renames.size ? `Moved ${renames.size} renamed ${renames.size === 1 ? 'file' : 'files'} from GitHub.` : '',
        recover.length
            ? `Kept ${recover.length} ${recover.length === 1 ? 'file with local edits' : 'files with local edits'} as independent copies.`
            : '',
    ]
        .filter(Boolean)
        .join(' ');
    return session.message;
}
