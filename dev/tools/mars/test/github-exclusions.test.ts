import assert from 'node:assert/strict';
import { test } from 'node:test';
import { excludedJobPath, githubExclusion, literalExclusion, visibleGitHubNodes } from '../src/github/exclusions.ts';
import type { TreeNode } from '../src/tree/model.ts';
import { githubIcon } from '../src/ui/icons.ts';

test('exclusions use case-sensitive gitignore rules and preserve literal context-menu paths', () => {
    const excludes = githubExclusion('# text dumps\n**/*.txt\n!/docs/keep.txt\n/archive/\n/README.md');
    assert.equal(excludes('docs/raw.txt'), true);
    assert.equal(excludes('docs/keep.txt'), false);
    assert.equal(excludes('archive/a.md'), true);
    assert.equal(excludes('archive'), false);
    assert.equal(excludes('docs/archive/a.md'), false);
    assert.equal(excludes('README.md'), true);
    assert.equal(excludes('docs/README.md'), false);
    assert.equal(excludes('Readme.md'), false);
    assert.equal(githubExclusion('/archive/\n!/archive/keep.md')('archive/keep.md'), true);

    const literal = githubExclusion(literalExclusion('archive/* [draft]? ', true));
    assert.equal(literal('archive/* [draft]? /one.md'), true);
    assert.equal(literal('archive/abc dX /one.md'), false);
    assert.equal(literal('archive/renamed/one.md'), false);
    for (const path of ['#note.md', '!note.md', 'file[?]*.md', 'file.md ']) {
        const match = githubExclusion(literalExclusion(path, false));
        assert.equal(match(path), true);
        assert.equal(match(`nested/${path}`), false);
    }
});

test('hidden cached files keep their identity and never leave an empty imported branch visible', () => {
    const make = (id: string, props: Partial<TreeNode>): TreeNode => ({
        id,
        name: id,
        kind: 'folder',
        parentId: 'repo',
        order: 0,
        createdAt: 1,
        updatedAt: 1,
        ...props,
    });
    const root = make('repo', {
        parentId: null,
        github: { repository: 'example/repo', branch: 'main', write: true },
        githubExcludes: '/archive/\n*.txt\n!/mixed/show.txt',
    });
    const nodes = [
        root,
        make('archive', {}),
        make('old', {
            parentId: 'archive',
            kind: 'doc',
            githubFile: { root: 'repo', path: 'archive/old.md', sha: 'x' },
        }),
        make('empty', { parentId: 'archive' }),
        make('logs', {}),
        make('log', { parentId: 'logs', kind: 'doc', githubFile: { root: 'repo', path: 'logs/read.txt', sha: 'x' } }),
        make('mixed', {}),
        make('show', {
            parentId: 'mixed',
            kind: 'doc',
            githubFile: { root: 'repo', path: 'mixed/show.txt', sha: 'x' },
        }),
        make('local', {}),
    ];
    assert.deepEqual(
        visibleGitHubNodes(nodes).map(({ id }) => id),
        ['repo', 'mixed', 'show', 'local'],
    );
    delete root.githubExcludes;
    assert.equal(visibleGitHubNodes(nodes), nodes);
});

test('a pending structural edit touches both its source and destination', () => {
    const job = {
        id: 'job',
        node: 'folder',
        kind: 'rename' as const,
        ids: ['folder', 'doc'],
        name: 'Old',
        from: 'Old',
        to: 'New',
        folder: true,
        files: [{ id: 'doc', from: 'Old/Guide.md', to: 'New/Guide.md', sha: 'old' }],
    };
    assert.equal(excludedJobPath(job, githubExclusion('/Old/')), 'Old/');
    assert.equal(excludedJobPath(job, githubExclusion('/New/Guide.md')), 'New/Guide.md');
    assert.equal(excludedJobPath(job, githubExclusion('/other/')), undefined);
});

test('the read-only GitHub icon keeps the full-size mark and adds a lock', () => {
    const bare = githubIcon(false);
    const locked = githubIcon(true);
    assert.ok(locked.startsWith(bare.slice(0, -'</svg>'.length)));
    assert.match(locked, /class="tree-github-lock"/);
});
