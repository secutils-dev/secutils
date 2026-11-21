// Browser-only access to GitHub. Tokens stay in this client and never follow redirects.
const ORIGIN = 'https://api.github.com';
const MAX_FILE_BYTES = 2 * 1024 * 1024;
const MAX_BATCH_BYTES = 4 * 1024 * 1024;
const oid = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);

/** Repository identity. The editable MARS folder label is not part of it. */
export interface GitHubSource {
    repository: string;
    branch: string;
}

/** A supported regular file in an immutable Git tree. */
export interface GitHubFile {
    path: string;
    sha: string;
    size: number;
}

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
    constructor(
        status: number,
        retryAfterMs = 0,
        message = 'GitHub rejected the request. Check access and try again.',
    ) {
        super(message);
        this.status = status;
        this.retryAfterMs = retryAfterMs;
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
    constructor(token: string, http: typeof fetch = (...args) => globalThis.fetch(...args)) {
        this.token = token;
        this.http = http;
    }

    private async request(path: string, signal?: AbortSignal, body?: unknown) {
        const response = await this.http(`${ORIGIN}${path}`, {
            redirect: 'error',
            credentials: 'omit',
            referrerPolicy: 'no-referrer',
            cache: 'no-store',
            method: body ? 'POST' : 'GET',
            signal,
            headers: {
                Accept: 'application/vnd.github+json',
                ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
                ...(body ? { 'Content-Type': 'application/json' } : {}),
            },
            ...(body ? { body: JSON.stringify(body) } : {}),
        });
        if (!response.ok) {
            const retry = Number(response.headers.get('retry-after') ?? 0) * 1000;
            const reset =
                response.headers.get('x-ratelimit-remaining') === '0'
                    ? Number(response.headers.get('x-ratelimit-reset') ?? 0) * 1000 - Date.now()
                    : 0;
            throw new GitHubError(
                response.status,
                Math.max(0, retry || 0, reset || 0),
                response.status === 401
                    ? 'GitHub authorization expired. Reconnect to continue.'
                    : response.status === 404
                      ? 'GitHub could not find this repository or branch. Check the name and token access.'
                      : response.status === 429 || retry > 0 || reset > 0
                        ? 'GitHub rate limited this connection. Try again later.'
                        : undefined,
            );
        }
        return response.json();
    }

    /** Resolve the default branch when no branch was specified, and keep its real name. */
    async resolve(source: GitHubSource, signal?: AbortSignal): Promise<GitHubSource> {
        const value = await this.request(`/repos/${source.repository}`, signal);
        if (typeof value.full_name !== 'string' || typeof value.default_branch !== 'string')
            throw new Error('Invalid GitHub repository response.');
        return githubSource(value.full_name, source.branch || value.default_branch);
    }

    /** A small branch lookup. An unchanged commit needs no tree or file requests. */
    async head(source: GitHubSource, signal?: AbortSignal): Promise<{ commit: string; tree: string }> {
        const value = await this.request(
            `/repos/${source.repository}/branches/${encodeURIComponent(source.branch)}`,
            signal,
        );
        const commit = value.commit?.sha,
            tree = value.commit?.commit?.tree?.sha;
        if (!oid(commit) || !oid(tree)) throw new Error('Invalid GitHub branch response.');
        return { commit, tree };
    }

    /** List only safe regular text blobs. Reject an incomplete listing before applying any changes. */
    async files(source: GitHubSource, tree: string, signal?: AbortSignal): Promise<GitHubFile[]> {
        const value = await this.request(`/repos/${source.repository}/git/trees/${tree}?recursive=1`, signal);
        if (value.truncated || !Array.isArray(value.tree))
            throw new Error('GitHub did not return a complete repository tree.');
        return value.tree
            .filter(
                (row: any) =>
                    row.type === 'blob' &&
                    row.mode === '100644' &&
                    validPath(row.path) &&
                    /\.(?:md|markdown|mdx|txt|html?)$/i.test(row.path) &&
                    oid(row.sha) &&
                    Number.isSafeInteger(row.size) &&
                    row.size >= 0 &&
                    row.size <= MAX_FILE_BYTES,
            )
            .map(({ path, sha, size }: GitHubFile) => ({ path, sha, size }));
    }

    /** Read the exact Git blob, preserving UTF-8 BOM and line endings. */
    async text(source: GitHubSource, file: GitHubFile, signal?: AbortSignal): Promise<string | null> {
        const value = await this.request(`/repos/${source.repository}/git/blobs/${file.sha}`, signal);
        if (
            value.sha !== file.sha ||
            value.encoding !== 'base64' ||
            typeof value.content !== 'string' ||
            value.content.length > MAX_FILE_BYTES * 2
        )
            throw new Error('GitHub returned an invalid file.');
        let result: string;
        try {
            result = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(
                Uint8Array.from(atob(value.content), (c) => c.charCodeAt(0)),
            );
        } catch {
            return null;
        }
        return result.includes('\0') || /^version https:\/\/git-lfs\.github\.com\/spec\/v1\r?\n/.test(result)
            ? null
            : result;
    }

    /** Publish existing text-file edits together. GitHub refuses an unexpected branch head. */
    async commit(
        source: GitHubSource,
        expectedHeadOid: string,
        edits: { path: string; text: string }[],
        signal?: AbortSignal,
    ) {
        if (!this.token || !edits.length || !oid(expectedHeadOid))
            throw new Error('Connect with a write token before syncing edits.');
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
                    branch: { repositoryNameWithOwner: source.repository, refName: `refs/heads/${source.branch}` },
                    expectedHeadOid,
                    message: { headline: 'Sync document edits from MARS' },
                    fileChanges: { additions },
                },
            },
        });
        const result = value.data?.createCommitOnBranch?.commit?.oid;
        if (!oid(result) || value.errors?.length)
            throw new GitHubError(409, 0, 'GitHub did not accept the batch. Check branch access and sync again.');
        return result;
    }
}
