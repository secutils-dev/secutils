import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { GitHubApi, gitBlobOid, githubSource } from '../src/github/api.ts';
import { fileDecision } from '../src/github/merge.ts';

const oid = '1'.repeat(40);
const next = '2'.repeat(40);
const json = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status });

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
    assert.equal(request.variables.input.branch.refName, 'refs/heads/topic/test');
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

test('a rejected commit never exposes GitHub response data or retries with stronger permissions', async () => {
    const api = new GitHubApi('test-token', async () => json({ errors: [{ message: 'sensitive response data' }] }));
    await assert.rejects(
        api.commit(githubSource('o/r', 'main'), oid, [{ path: 'a.md', text: 'x' }]),
        (error: Error) => !error.message.includes('sensitive') && /did not accept/.test(error.message),
    );
});
