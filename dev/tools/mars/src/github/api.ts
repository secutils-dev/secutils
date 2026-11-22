// Browser-only access to GitHub. Tokens stay in this client and never follow redirects.
const ORIGIN = 'https://api.github.com';
const RAW_ORIGIN = 'https://raw.githubusercontent.com';
const MAX_FILE_BYTES = 2 * 1024 * 1024;
const MAX_BATCH_BYTES = 4 * 1024 * 1024;
const REQUEST_TIMEOUT_MS = 30_000;
const MAX_BLOB_QUERY_BYTES = 1024 * 1024;
const MAX_BLOB_QUERY_FILES = 24;
const oid = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);

/** Repository identity. The editable MARS folder label is not part of it. */
export interface GitHubSource {
    repository: string;
    branch: string;
    public?: boolean;
}

/** A supported regular file in an immutable Git tree. */
export interface GitHubFile {
    path: string;
    sha: string;
    size: number;
}

/** An entry in the complete Git tree, including paths MARS must not overwrite or delete. */
export interface GitHubTreeEntry {
    path: string;
    sha: string;
    mode: string;
    type: string;
    size?: number;
}

/** Branch response and its HTTP validator. A locally committed head has no validator until checked. */
export interface GitHubHead {
    commit: string;
    tree: string | null;
    etag?: string;
}

const validEtag = (value: unknown): value is string =>
    typeof value === 'string' && value.length <= 512 && /^(?:W\/)?"[\x21\x23-\x7e]*"$/.test(value);
const requestSignal = (signal?: AbortSignal) => {
    const timeout = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
    return signal ? AbortSignal.any([signal, timeout]) : timeout;
};

/** Validates the repository and optional branch without accepting an arbitrary request origin. */
export function githubSource(repository: string, branch: string): GitHubSource {
    const name = repository
        .trim()
        .replace(/^https:\/\/github\.com\//i, '')
        .replace(/\/$/, '');
    if (!/^[a-z\d](?:[a-z\d-]{0,38})\/[a-z\d._-]{1,100}$/i.test(name) || /\/(?:\.|\.\.)$/.test(name))
        throw new Error('Enter a GitHub repository as owner/repository.');
    const ref = branch.trim();
    if (
        ref.length > 255 ||
        ref.split('/').some((part) => part === '.' || part === '..') ||
        /[\x00-\x20\x7f~^:?*\[\\]/.test(ref)
    )
        throw new Error('Enter a valid GitHub branch name.');
    return { repository: name, branch: ref };
}

/** True for text formats that MARS can write back without HTML conversion. */
export const writablePath = (path: string) => /\.(?:md|markdown|mdx|txt)$/i.test(path);

/** The Git blob identifier for these exact bytes; SHA-1 is Git's format, not an authorization check. */
export async function gitBlobOid(text: string) {
    const encoder = new TextEncoder(),
        bytes = encoder.encode(text);
    const header = encoder.encode(`blob ${bytes.length}\0`),
        git = new Uint8Array(header.length + bytes.length);
    git.set(header);
    git.set(bytes, header.length);
    const digest = await crypto.subtle.digest('SHA-1', git);
    return Array.from(new Uint8Array(digest), (value) => value.toString(16).padStart(2, '0')).join('');
}
const validPath = (path: unknown): path is string =>
    typeof path === 'string' &&
    path.length <= 4096 &&
    !/[\\\x00-\x1f\x7f]/.test(path) &&
    path.split('/').every((part) => part && part !== '.' && part !== '..');

/** If a commit fails, report a fixed message and allow Retry without exposing a token or response body. */
export class GitHubError extends Error {
    readonly status: number;
    readonly retryAfterMs: number;
    readonly rateLimited: boolean;
    constructor(
        status: number,
        retryAfterMs = 0,
        message = 'GitHub rejected the request. Check access and try again.',
        rateLimited = false,
    ) {
        super(message);
        this.status = status;
        this.retryAfterMs = retryAfterMs;
        this.rateLimited = rateLimited;
    }
}

const base64 = (bytes: Uint8Array) => {
    let binary = '';
    for (let i = 0; i < bytes.length; i += 8192) binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
    return btoa(binary);
};

/** Reads trees and blobs, and publishes one atomic GitHub commit against an expected branch head. */
export class GitHubApi {
    private readonly token: string;
    private readonly http: typeof fetch;
    private readonly priority: RequestPriority;
    constructor(
        token: string,
        http: typeof fetch = (...args) => globalThis.fetch(...args),
        priority: RequestPriority = 'auto',
    ) {
        this.token = token;
        this.http = http;
        this.priority = priority;
    }

    private async response(path: string, signal?: AbortSignal, body?: unknown, etag?: string) {
        const response = await this.http(`${ORIGIN}${path}`, {
            redirect: 'error',
            credentials: 'omit',
            referrerPolicy: 'no-referrer',
            cache: 'no-store',
            method: body ? 'POST' : 'GET',
            signal: requestSignal(signal),
            priority: this.priority,
            headers: {
                Accept: 'application/vnd.github+json',
                ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
                ...(body ? { 'Content-Type': 'application/json' } : {}),
                ...(etag ? { 'If-None-Match': etag } : {}),
            },
            ...(body ? { body: JSON.stringify(body) } : {}),
        });
        if (response.status === 304 && etag) return response;
        if (!response.ok) {
            const retry = Number(response.headers.get('retry-after') ?? 0) * 1000;
            const reset =
                response.headers.get('x-ratelimit-remaining') === '0'
                    ? Number(response.headers.get('x-ratelimit-reset') ?? 0) * 1000 - Date.now()
                    : 0;
            const limited =
                response.status === 429 ||
                (response.status === 403 &&
                    (retry > 0 ||
                        reset > 0 ||
                        response.headers.get('x-ratelimit-remaining') === '0' ||
                        /secondary rate limit/i.test((await response.json().catch(() => null))?.message ?? '')));
            const until =
                reset > 0 && Number.isFinite(reset)
                    ? ` Retry after ${new Date(Date.now() + reset).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}.`
                    : ' Try again in a few minutes.';
            throw new GitHubError(
                response.status,
                Math.max(0, retry || 0, reset || 0),
                response.status === 401
                    ? 'GitHub authorization expired. Reconnect to continue.'
                    : response.status === 404
                      ? this.token
                          ? 'GitHub could not find this repository or branch. Check the name, branch, and whether your token has access.'
                          : 'GitHub could not find a public repository or branch at that address. Check the repository name and branch.'
                      : limited
                        ? this.token
                            ? `GitHub paused requests for this token.${until}`
                            : `GitHub's anonymous limit for this IP address is exhausted. Add a repository-scoped token to continue now.${until}`
                        : undefined,
                limited,
            );
        }
        return response;
    }

    private async request(path: string, signal?: AbortSignal, body?: unknown) {
        return (await this.response(path, signal, body)).json();
    }

    /** Resolve the default branch when no branch was specified, and keep its real name. */
    async resolve(source: GitHubSource, signal?: AbortSignal): Promise<GitHubSource> {
        const value = await this.request(`/repos/${source.repository}`, signal);
        if (typeof value.full_name !== 'string' || typeof value.default_branch !== 'string')
            throw new Error('Invalid GitHub repository response.');
        return {
            ...githubSource(value.full_name, source.branch || value.default_branch),
            public: value.private === false,
        };
    }

    /** A small branch lookup. An unchanged commit needs no tree or file requests. */
    async head(source: GitHubSource, signal?: AbortSignal, previous?: GitHubHead): Promise<GitHubHead> {
        const etag = validEtag(previous?.etag) ? previous.etag : undefined;
        const response = await this.response(
            `/repos/${source.repository}/branches/${encodeURIComponent(source.branch)}`,
            signal,
            undefined,
            etag,
        );
        if (response.status === 304 && previous) return previous;
        const value = await response.json();
        const commit = value.commit?.sha,
            tree = value.commit?.commit?.tree?.sha;
        if (!oid(commit) || !oid(tree)) throw new Error('Invalid GitHub branch response.');
        const nextEtag = response.headers.get('etag');
        return { commit, tree, ...(validEtag(nextEtag) ? { etag: nextEtag } : {}) };
    }

    /** Read the complete tree. A truncated listing cannot be used for writes. */
    async tree(source: GitHubSource, tree: string, signal?: AbortSignal): Promise<GitHubTreeEntry[]> {
        const value = await this.request(`/repos/${source.repository}/git/trees/${tree}?recursive=1`, signal);
        if (
            value.truncated ||
            !Array.isArray(value.tree) ||
            value.tree.some(
                (row: Partial<GitHubTreeEntry> | null) =>
                    typeof row?.path !== 'string' ||
                    !oid(row.sha) ||
                    typeof row.mode !== 'string' ||
                    typeof row.type !== 'string',
            )
        )
            throw new Error('GitHub did not return a complete repository tree.');
        return value.tree;
    }

    /** List only safe regular text blobs. */
    async files(source: GitHubSource, tree: string, signal?: AbortSignal): Promise<GitHubFile[]> {
        return (await this.tree(source, tree, signal))
            .filter(
                (row): row is GitHubTreeEntry & GitHubFile =>
                    row.type === 'blob' &&
                    row.mode === '100644' &&
                    validPath(row.path) &&
                    /\.(?:md|markdown|mdx|txt|html?)$/i.test(row.path) &&
                    oid(row.sha) &&
                    typeof row.size === 'number' &&
                    Number.isSafeInteger(row.size) &&
                    row.size >= 0 &&
                    row.size <= MAX_FILE_BYTES,
            )
            .map(({ path, sha, size }: GitHubFile) => ({ path, sha, size }));
    }

    /** Optional rename hints for edited moves. A rewritten or unavailable history cannot block tree sync. */
    async renameHints(source: GitHubSource, previous: string, next: string, signal?: AbortSignal) {
        if (!oid(previous) || !oid(next) || previous === next) return [];
        try {
            const value = await this.request(`/repos/${source.repository}/compare/${previous}...${next}`, signal);
            // GitHub compares with the merge base for divergent histories, not with the last tree MARS saw.
            if (value.status !== 'ahead' || value.merge_base_commit?.sha !== previous || !Array.isArray(value.files))
                return [];
            return value.files.flatMap((entry: any) =>
                entry.status === 'renamed' && validPath(entry.previous_filename) && validPath(entry.filename)
                    ? [{ from: entry.previous_filename as string, to: entry.filename as string }]
                    : [],
            );
        } catch (error) {
            signal?.throwIfAborted();
            return [];
        }
    }

    /** Batch authenticated text reads. Fall back per blob if GraphQL omits or changes its bytes. */
    async textBatch(source: GitHubSource, files: GitHubFile[], signal?: AbortSignal): Promise<Map<string, string>> {
        if (
            !this.token ||
            files.length < 2 ||
            files.length > MAX_BLOB_QUERY_FILES ||
            files.reduce((sum, file) => sum + file.size, 0) > MAX_BLOB_QUERY_BYTES
        )
            return new Map();
        const variables: Record<string, string> = {};
        [variables.owner, variables.name] = source.repository.split('/');
        const fields = files.map((file, index) => {
            variables[`oid${index}`] = file.sha;
            return `b${index}:object(oid:$oid${index}){... on Blob{oid isTruncated isBinary text}}`;
        });
        const definitions = files.map((_, index) => `$oid${index}:GitObjectID!`).join(',');
        const value = await this.request('/graphql', signal, {
            query: `query MarsBlobs($owner:String!,$name:String!,${definitions}){repository(owner:$owner,name:$name){${fields.join(' ')}}}`,
            variables,
        });
        const result = new Map<string, string>();
        await Promise.all(
            files.map(async (file, index) => {
                const entry = value.data?.repository?.[`b${index}`];
                if (
                    !entry ||
                    entry.oid !== file.sha ||
                    entry.isTruncated ||
                    entry.isBinary ||
                    typeof entry.text !== 'string' ||
                    new TextEncoder().encode(entry.text).length !== file.size ||
                    (await gitBlobOid(entry.text)) !== file.sha
                )
                    return;
                if (
                    !entry.text.includes('\0') &&
                    !/^version https:\/\/git-lfs\.github\.com\/spec\/v1\r?\n/.test(entry.text)
                )
                    result.set(file.sha, entry.text);
            }),
        );
        return result;
    }

    /** Read the exact Git blob, preserving UTF-8 BOM and line endings. */
    async text(source: GitHubSource, file: GitHubFile, signal?: AbortSignal, commit?: string): Promise<string | null> {
        let bytes: Uint8Array;
        if (source.public && oid(commit)) {
            const path = file.path.split('/').map(encodeURIComponent).join('/');
            // Public content is immutable at this commit. Do not spend API quota or send a PAT for the file bytes.
            const response = await this.http(`${RAW_ORIGIN}/${source.repository}/${commit}/${path}`, {
                redirect: 'error',
                credentials: 'omit',
                referrerPolicy: 'no-referrer',
                signal: requestSignal(signal),
                priority: this.priority,
            });
            if (!response.ok)
                throw new GitHubError(response.status, 0, 'Could not download the public file from GitHub. Try again.');
            const stream = response.body?.getReader(),
                buffer = new Uint8Array(file.size);
            if (!stream) throw new Error('GitHub returned an empty file response.');
            let length = 0;
            try {
                while (true) {
                    const { done, value } = await stream.read();
                    if (done) break;
                    if (length + value.byteLength > buffer.length)
                        throw new Error('GitHub returned a file of the wrong size.');
                    buffer.set(value, length);
                    length += value.byteLength;
                }
                if (length !== file.size) throw new Error('GitHub returned a file of the wrong size.');
            } finally {
                void stream.cancel().catch(() => {});
            }
            bytes = buffer;
        } else {
            const value = await this.request(`/repos/${source.repository}/git/blobs/${file.sha}`, signal);
            if (
                value.sha !== file.sha ||
                value.encoding !== 'base64' ||
                typeof value.content !== 'string' ||
                value.content.length > MAX_FILE_BYTES * 2
            )
                throw new Error('GitHub returned an invalid file.');
            try {
                bytes = Uint8Array.from(atob(value.content), (c) => c.charCodeAt(0));
            } catch {
                return null;
            }
        }
        let result: string;
        try {
            result = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
        } catch {
            return null;
        }
        return result.includes('\0') || /^version https:\/\/git-lfs\.github\.com\/spec\/v1\r?\n/.test(result)
            ? null
            : result;
    }

    /** Publish text changes together; GitHub atomically checks the expected branch head. */
    async commit(
        source: GitHubSource,
        expectedHeadOid: string,
        edits: { path: string; text: string }[],
        signal?: AbortSignal,
        deletedPaths: string[] = [],
    ) {
        if (!this.token || (!edits.length && !deletedPaths.length) || !oid(expectedHeadOid))
            throw new Error('Connect with a write token before syncing edits.');
        const paths = [...edits.map(({ path }) => path), ...deletedPaths];
        if (paths.some((path) => !validPath(path) || !writablePath(path)) || new Set(paths).size !== paths.length)
            throw new Error('These file paths cannot be written back to GitHub.');
        let total = 0;
        const additions = edits.map(({ path, text }) => {
            if (!validPath(path) || !writablePath(path)) throw new Error('This file cannot be written back to GitHub.');
            const bytes = new TextEncoder().encode(text);
            total += bytes.length;
            if (bytes.length > MAX_FILE_BYTES || total > MAX_BATCH_BYTES)
                throw new Error('This edit batch is too large for the GitHub preview.');
            return { path, contents: base64(bytes) };
        });
        const value = await this.request('/graphql', signal, {
            query: 'mutation MarsCommit($input:CreateCommitOnBranchInput!){createCommitOnBranch(input:$input){commit{oid}}}',
            variables: {
                input: {
                    branch: { repositoryNameWithOwner: source.repository, branchName: source.branch },
                    expectedHeadOid,
                    message: { headline: 'docs(mars): sync document edits' },
                    fileChanges: {
                        additions,
                        ...(deletedPaths.length ? { deletions: deletedPaths.map((path) => ({ path })) } : {}),
                    },
                },
            },
        });
        const result = value.data?.createCommitOnBranch?.commit?.oid;
        if (!oid(result) || value.errors?.length) {
            const errors = Array.isArray(value.errors) ? value.errors : [];
            const messages = errors.flatMap((error: { message?: unknown }) =>
                typeof error?.message === 'string' ? [error.message] : [],
            );
            if (
                messages.some((message: string) =>
                    /(?:expected|current|actual) (?:head|branch)|branch (?:head )?(?:has changed|advanced)/i.test(
                        message,
                    ),
                )
            )
                throw new GitHubError(
                    409,
                    0,
                    'The GitHub branch changed before the edit could be sent. Sync again to check the new version.',
                );
            if (
                errors.some((error: { type?: string }) => error?.type === 'FORBIDDEN') ||
                messages.some((message: string) =>
                    /protected branch|branch protection|resource not accessible|not permitted|permission|repository rule/i.test(
                        message,
                    ),
                )
            )
                throw new GitHubError(
                    403,
                    0,
                    'GitHub did not allow this write. Check that the token has Contents: Read and write and that the branch allows direct edits.',
                );
            throw new GitHubError(422, 0, 'GitHub did not accept the batch. Check branch access and sync again.');
        }
        return result;
    }
}
