import assert from 'node:assert/strict';
import { test } from 'node:test';
import { findGitHubRenames, similarGitHubRenames } from '../src/github/reconcile.ts';

const file = (path: string, sha: string) => ({ path, sha, size: 10 });

test('identifies unchanged blobs across renamed folders and keeps unchanged paths paired by name', () => {
    const old = [file('Old/A.md', 'a'), file('Old/Nested/B.md', 'b'), file('Stable.md', 's')];
    const next = [file('New/A.md', 'a'), file('New/Nested/B.md', 'b'), file('Stable.md', 'new')];
    assert.deepEqual(
        findGitHubRenames(old, next),
        new Map([
            ['New/A.md', 'Old/A.md'],
            ['New/Nested/B.md', 'Old/Nested/B.md'],
        ]),
    );
});

test('uses an upstream rename hint for changed bytes but never invents identities for ambiguous matches', () => {
    const old = [file('One.md', 'same'), file('Two.md', 'same'), file('Old.md', 'before')];
    const next = [file('Other.md', 'same'), file('New.md', 'after')];
    assert.deepEqual(findGitHubRenames(old, next, [{ from: 'Old.md', to: 'New.md' }]), new Map([['New.md', 'Old.md']]));
});

test('matches distinctive edited content without a history diff, but rejects tied candidates', () => {
    const original = '# Project onboarding\nInstall the required tools.\nUse the workspace directory.\nOld line.\n';
    const revised = original.replace('Old line.', 'New line.');
    assert.deepEqual(similarGitHubRenames(new Map([['Old.md', original]]), new Map([['New.md', revised]])), [
        { from: 'Old.md', to: 'New.md' },
    ]);
    assert.deepEqual(
        similarGitHubRenames(
            new Map([['Old.md', original]]),
            new Map([
                ['New.md', revised],
                ['Other.md', revised],
            ]),
        ),
        [],
    );
});
