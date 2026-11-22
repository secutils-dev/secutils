import assert from 'node:assert/strict';
import { test } from 'node:test';
import { planGitHubStructure, upstreamRename, type StructuralJob } from '../src/github/structure.ts';
import type { GitHubTreeEntry } from '../src/github/api.ts';

const blob = (path: string, sha: string): GitHubTreeEntry => ({ path, sha, mode: '100644', type: 'blob' });
const tree = (...files: GitHubTreeEntry[]) => new Map(files.map((file) => [file.path, file]));

test('two folder names are reported as one upstream rename conflict', () => {
    const job: StructuralJob = {
        id: 'folder',
        node: 'folder',
        ids: ['folder', 'a', 'b'],
        kind: 'rename',
        name: 'agent-guides',
        from: 'dev/agent-guides',
        to: 'dev/agent-guides-mars',
        folder: true,
        files: [
            { id: 'a', from: 'dev/agent-guides/A.md', to: 'dev/agent-guides-mars/A.md', sha: 'blob-a' },
            { id: 'b', from: 'dev/agent-guides/nested/B.md', to: 'dev/agent-guides-mars/nested/B.md', sha: 'blob-b' },
        ],
    };
    const upstream = tree(
        blob('dev/agent-guides-repo/A.md', 'blob-a'),
        blob('dev/agent-guides-repo/nested/B.md', 'blob-b'),
    );
    assert.equal(upstreamRename(job, upstream), 'dev/agent-guides-repo');
    assert.deepEqual(planGitHubStructure(job, upstream), {
        blocked: { reason: 'renamed', path: 'dev/agent-guides-repo' },
    });
    upstream.set('dev/duplicate/A.md', blob('dev/duplicate/A.md', 'blob-a'));
    upstream.set('dev/duplicate/nested/B.md', blob('dev/duplicate/nested/B.md', 'blob-b'));
    assert.equal(upstreamRename(job, upstream), undefined);
});
const rename: StructuralJob = {
    id: '1',
    node: 'a',
    ids: ['a'],
    kind: 'rename',
    name: 'A',
    folder: false,
    from: 'A.md',
    to: 'B.md',
    files: [{ id: 'a', from: 'A.md', to: 'B.md', sha: 'old', attemptedSha: 'ours' }],
};

test('structural renames do not overwrite a destination or an upstream edit', () => {
    assert.deepEqual(planGitHubStructure(rename, tree(blob('A.md', 'old'), blob('B.md', 'other'))), {
        blocked: { reason: 'destination', path: 'B.md' },
    });
    assert.deepEqual(planGitHubStructure(rename, tree(blob('A.md', 'new'))), {
        blocked: { reason: 'source', path: 'A.md', sha: 'new' },
    });
    assert.deepEqual(planGitHubStructure(rename, tree(blob('A.md', 'old'))), {
        complete: false,
        changes: rename.files,
    });
    assert.deepEqual(planGitHubStructure(rename, tree(blob('B.md', 'ours'))), { complete: true });
    assert.deepEqual(planGitHubStructure(rename, tree(blob('B.md', 'other'))), {
        blocked: { reason: 'source', path: 'A.md' },
    });
});

test('a deletion can only send the exact approved blob and does not delete unimported folder contents', () => {
    const job: StructuralJob = {
        ...rename,
        kind: 'delete',
        folder: true,
        from: 'Archive',
        to: undefined,
        files: [{ id: 'a', from: 'Archive/A.md', sha: 'approved' }],
    };
    assert.deepEqual(planGitHubStructure(job, tree(blob('Archive/A.md', 'newer'))), {
        blocked: { reason: 'source', path: 'Archive/A.md', sha: 'newer' },
    });
    assert.deepEqual(
        planGitHubStructure(job, tree(blob('Archive/A.md', 'approved'), blob('Archive/image.png', 'other'))),
        { blocked: { reason: 'outside', path: 'Archive/image.png' } },
    );
    assert.deepEqual(planGitHubStructure(job, tree(blob('Archive/A.md', 'approved'))), {
        complete: false,
        changes: job.files,
    });
    assert.deepEqual(planGitHubStructure(job, tree()), { complete: true });
});
