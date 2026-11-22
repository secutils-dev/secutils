import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { GitHubApi, gitBlobOid, githubSource } from '../src/github/api.ts';
import { fileDecision, mergeGitHubText } from '../src/github/merge.ts';
import { openGitHubToken, sealGitHubToken } from '../src/github/credentials.ts';

const oid = '1'.repeat(40);
const next = '2'.repeat(40);
const json = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status });

test('three-way merge combines independent edits with exact line endings and final newline', () => {
    assert.deepEqual(mergeGitHubText('One\r\nTwo\r\nThree', 'First\r\nTwo\r\nThree', 'One\r\nTwo\r\nLast'), [
        { text: 'First\r\nTwo\r\nLast' },
    ]);
    assert.deepEqual(mergeGitHubText('red and blue\n', 'green and blue\n', 'red and orange\n'), [
        { text: 'green and orange\n' },
    ]);
    assert.deepEqual(mergeGitHubText('red\r\nblue', 'green\r\nblue', 'red\r\norange'), [{ text: 'green\r\norange' }]);
    assert.deepEqual(mergeGitHubText('one\ntwo\n', 'insert\none\ntwo\n', 'one\n'), [{ text: 'insert\none\n' }]);
    assert.deepEqual(mergeGitHubText('same\nold\n', 'same\nnew\n', 'same\nnew\n'), [{ text: 'same\nnew\n' }]);
});

test('a costly merge preserves all three variants for manual review', () => {
    const repeated = 'repeat\n'.repeat(2000);
    const base = `base\n${repeated}old`,
        local = `local\n${repeated}old`,
        remote = `base\n${repeated}new`;
    assert.deepEqual(mergeGitHubText(base, local, remote), [{ base, local, remote }]);
});

test('three-way merge isolates contested ranges, including delete versus edit and same-position inserts', () => {
    assert.deepEqual(mergeGitHubText('one\nkeep\nthree\n', 'mine\nkeep\nthree\n', 'theirs\nkeep\nlast\n'), [
        { base: 'one\n', local: 'mine\n', remote: 'theirs\n' },
        { text: 'keep\nlast\n' },
    ]);
    assert.deepEqual(mergeGitHubText('one\ntwo\n', 'one\n', 'one\nchanged\n'), [
        { text: 'one\n' },
        { base: 'two\n', local: '', remote: 'changed\n' },
    ]);
    assert.deepEqual(mergeGitHubText('two\n', 'local\ntwo\n', 'remote\ntwo\n'), [
        { base: '', local: 'local\n', remote: 'remote\n' },
        { text: 'two\n' },
    ]);
});

test('authenticated text batching accepts only complete, byte-identical blobs', async () => {
    const text = '\ufeff# Exact bytes\r\n',
        sha = await gitBlobOid(text);
    const mismatched = { path: 'b.md', sha: next, size: 3 };
    const api = new GitHubApi('test-token', async (url, init) => {
        assert.equal(url, 'https://api.github.com/graphql');
        assert.equal(new Headers(init?.headers).get('authorization'), 'Bearer test-token');
        const query = JSON.parse(String(init?.body));
        assert.deepEqual(query.variables, { owner: 'owner', name: 'repo', oid0: sha, oid1: next });
        return json({
            data: {
                repository: {
                    b0: { oid: sha, text, isBinary: false, isTruncated: false },
                    b1: { oid: next, text: 'bad', isBinary: false, isTruncated: false },
                },
            },
        });
    });
    assert.deepEqual(
        await api.textBatch(githubSource('owner/repo', 'main'), [
            { path: 'a.md', size: new TextEncoder().encode(text).length, sha },
            mismatched,
        ]),
        new Map([[sha, text]]),
    );
});

test('authenticated branch checks use a saved ETag and reuse the cached head on 304', async () => {
    const source = githubSource('owner/repo', 'main');
    let requests = 0;
    const api = new GitHubApi(
        'test-token',
        async (_url, init) => {
            const headers = new Headers(init?.headers);
            assert.equal(headers.get('authorization'), 'Bearer test-token');
            assert.equal(init?.priority, 'low');
            if (++requests === 1) {
                assert.equal(headers.get('if-none-match'), null);
                return new Response(JSON.stringify({ commit: { sha: oid, commit: { tree: { sha: next } } } }), {
                    headers: { etag: 'W/"branch-head"' },
                });
            }
            assert.equal(headers.get('if-none-match'), 'W/"branch-head"');
            return new Response(null, { status: 304 });
        },
        'low',
    );
    const saved = await api.head(source);
    assert.equal(saved.etag, 'W/"branch-head"');
    assert.deepEqual(await api.head(source, undefined, structuredClone(saved)), saved);
});

test('saved PAT survives a structured clone with a non-exportable, folder-bound key', async () => {
    const credential = structuredClone(await sealGitHubToken('private-test-token', 'root'));
    assert.equal(credential.key.extractable, false);
    assert.equal(await openGitHubToken(credential, 'root'), 'private-test-token');
    await assert.rejects(openGitHubToken(credential, 'other-root'));
});

test('public files use immutable raw URLs without API authentication', async () => {
    const bytes = new TextEncoder().encode('# Zürich\r\n');
    const api = new GitHubApi('test-token', async (url, init) => {
        assert.equal(url, `https://raw.githubusercontent.com/owner/repo/${oid}/Nested/A%20%23.md`);
        assert.equal(new Headers(init?.headers).get('Authorization'), null);
        return new Response(bytes);
    });
    assert.equal(
        await api.text(
            { repository: 'owner/repo', branch: 'main', public: true },
            { path: 'Nested/A #.md', sha: next, size: bytes.length },
            undefined,
            oid,
        ),
        '# Zürich\r\n',
    );
});

test('anonymous exhaustion explains IP quota instead of suggesting a permissions error', async () => {
    const api = new GitHubApi(
        '',
        async () =>
            new Response(null, {
                status: 403,
                headers: {
                    'x-ratelimit-remaining': '0',
                    'x-ratelimit-reset': String(Math.floor(Date.now() / 1000) + 180),
                },
            }),
    );
    await assert.rejects(
        api.resolve(githubSource('owner/repo', '')),
        /anonymous limit for this IP address.*Add a repository-scoped token/,
    );
});

test('acknowledged blob IDs match Git framing for UTF-8 and CRLF', async () => {
    const text = '\ufeff# Zürich\r\n';
    assert.equal(
        await gitBlobOid(text),
        createHash('sha1')
            .update(`blob ${Buffer.byteLength(text)}\0${text}`)
            .digest('hex'),
    );
});

test('validates GitHub sources and protects divergent edits', () => {
    assert.deepEqual(githubSource('https://github.com/Owner/Repo/', 'a/topic'), {
        repository: 'Owner/Repo',
        branch: 'a/topic',
    });
    for (const bad of ['https://example.com/Owner/Repo', 'Owner/..', 'Owner/Repo?token=abc', 'Owner/Repo/more'])
        assert.throws(() => githubSource(bad, 'main'));
    assert.equal(fileDecision('base', 'base', 'remote'), 'pull');
    assert.equal(fileDecision('base', 'local', 'base'), 'push');
    assert.equal(fileDecision('base', 'local', 'remote'), 'conflict');
    assert.equal(fileDecision('base', 'same', 'same'), 'ack');
});

test('reads only supported regular files and commits both edits only to the chosen GitHub branch', async () => {
    const source = githubSource('owner/repo', 'topic/test');
    const calls: { url: string; init: RequestInit }[] = [];
    const api = new GitHubApi('test-token', async (url, init) => {
        calls.push({ url: String(url), init: init! });
        if (String(url).includes('/branches/')) return json({ commit: { sha: oid, commit: { tree: { sha: next } } } });
        if (String(url).includes('/trees/'))
            return json({
                tree: [
                    { path: 'Nested/A.md', mode: '100644', type: 'blob', sha: oid, size: 11 },
                    { path: 'Link.md', mode: '120000', type: 'blob', sha: oid, size: 7 },
                    { path: 'ignore.js', mode: '100644', type: 'blob', sha: oid, size: 7 },
                    { path: '../unsafe.md', mode: '100644', type: 'blob', sha: oid, size: 7 },
                ],
            });
        if (String(url).includes('/blobs/'))
            return json({ sha: oid, encoding: 'base64', content: Buffer.from('\ufeff# hello\r\n').toString('base64') });
        return json({ data: { createCommitOnBranch: { commit: { oid: next } } } });
    });
    const head = await api.head(source);
    assert.ok(calls[0].url.endsWith('/branches/topic%2Ftest'));
    const files = await api.files(source, head.tree);
    assert.deepEqual(
        files.map((file) => file.path),
        ['Nested/A.md'],
    );
    assert.equal(await api.text(source, files[0]), '\ufeff# hello\r\n');
    assert.equal(
        await api.commit(source, head.commit, [
            { path: 'Nested/A.md', text: 'one' },
            { path: 'B.txt', text: 'two' },
        ]),
        next,
    );
    const request = JSON.parse(String(calls.at(-1)!.init.body));
    assert.equal(request.variables.input.expectedHeadOid, oid);
    assert.deepEqual(request.variables.input.message, { headline: 'docs(mars): sync document edits' });
    assert.deepEqual(request.variables.input.branch, {
        repositoryNameWithOwner: 'owner/repo',
        branchName: 'topic/test',
    });
    assert.deepEqual(request.variables.input.fileChanges.additions, [
        { path: 'Nested/A.md', contents: 'b25l' },
        { path: 'B.txt', contents: 'dHdv' },
    ]);
    assert.ok(
        calls.every(({ url, init }) => new URL(url).origin === 'https://api.github.com' && init.redirect === 'error'),
    );
    await assert.rejects(api.commit(source, head.commit, [{ path: 'page.html', text: 'do not rewrite' }]));
    assert.equal(calls.length, 4);
});

test('commit errors identify write permission failures without echoing upstream data', async () => {
    const api = new GitHubApi('test-token', async () =>
        json({ errors: [{ type: 'FORBIDDEN', message: 'secret upstream details' }] }),
    );
    await assert.rejects(
        api.commit(githubSource('o/r', 'main'), oid, [{ path: 'a.md', text: 'x' }]),
        (error: Error) => /Contents: Read and write/.test(error.message) && !error.message.includes('secret'),
    );
});

test('a rejected commit never exposes GitHub response data or retries with stronger permissions', async () => {
    const api = new GitHubApi('test-token', async () => json({ errors: [{ message: 'sensitive response data' }] }));
    await assert.rejects(
        api.commit(githubSource('o/r', 'main'), oid, [{ path: 'a.md', text: 'x' }]),
        (error: Error) => !error.message.includes('sensitive') && /did not accept/.test(error.message),
    );
});
