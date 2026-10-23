// Logic checks for the pure modules: `npm test` (Node >= 22, type stripping).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { decodeState, encodeState, openSealed, parseBundle, sealBundle, type Bundle } from '../src/share/codec.ts';
import { joinComments, mergeDocuments, splitComments, type Thread } from '../src/share/merge.ts';
import { canMove, childrenOf, exportPaths, moveNode, type TreeNode } from '../src/tree/model.ts';

test('URL state round-trips and matches the md.html wire format', async () => {
    const text = '# Hi\n\nÜnïcödé ✓ and `code`\n';
    const encoded = await encodeState(text);
    assert.equal(await decodeState(encoded), text);
    const raw = Buffer.from(encoded, 'base64url');
    assert.equal(raw.readUInt32LE(0), Buffer.byteLength(text));
    assert.equal(await decodeState('not-a-state'), null);
});

test('sealed bundles open only with their secret', async () => {
    const bundle: Bundle = { v: 1, kind: 'folder', name: 'RFC', originId: 'f1', docs: [{ path: ['a'], name: 'One', originId: 'd1', text: '# One' }] };
    const { id, secret, blob } = await sealBundle(bundle);
    assert.match(id, /^[A-Za-z0-9_-]{16}$/);
    assert.deepEqual(await openSealed(secret, blob), bundle);
    const other = await sealBundle(bundle);
    assert.equal(await openSealed(other.secret, blob), null);
    assert.equal(parseBundle('{"v":1,"kind":"doc","name":"x","docs":[{"name":1}]}'), null);
});

const node = (id: string, parentId: string | null, kind: TreeNode['kind'] = 'doc', order = 0): TreeNode =>
    ({ id, parentId, kind, name: id, order, createdAt: 0, updatedAt: 0 });

test('tree moves reorder siblings and never create cycles', () => {
    const nodes = [node('f', null, 'folder', 0), node('g', 'f', 'folder', 0), node('a', null, 'doc', 1), node('b', null, 'doc', 2)];
    assert.equal(canMove(nodes, 'f', 'g'), false);
    assert.deepEqual(moveNode(nodes, 'f', 'g'), []);
    moveNode(nodes, 'b', null, 'f');
    assert.deepEqual(childrenOf(nodes, null).map((n) => n.id), ['b', 'f', 'a']);
    moveNode(nodes, 'a', 'g');
    assert.deepEqual(childrenOf(nodes, 'g').map((n) => n.id), ['a']);
    const paths = exportPaths([...nodes, node('a2', 'g', 'doc', 1)].map((n) => (n.id === 'a2' ? { ...n, name: 'a' } : n)), null);
    assert.equal(paths.get('a'), 'f/g/a.md');
    assert.equal(paths.get('a2'), 'f/g/a (2).md');
});

test('comment merge unions threads and comments and keeps the body choice', () => {
    const t = (id: string, comments: Thread['comments']): Thread => ({ id, quote: 'q', comments });
    const mine = joinComments('# Doc\n', [t('t1', [{ id: 'c1', author: 'A', at: '2026-01-01', body: 'one' }])]);
    const theirs = joinComments('# Doc\n', [
        t('t1', [{ id: 'c1', author: 'A', at: '2026-01-01', body: 'one' }, { id: 'c2', author: 'B', at: '2026-01-02', body: 'two' }]),
        t('t2', [{ author: 'B', at: '2026-01-03', body: 'new thread' }]),
    ]);
    const merged = mergeDocuments(mine, theirs);
    assert.equal(merged.sameBody, true);
    assert.equal(merged.added, 2);
    const { body, threads } = splitComments(merged.mine);
    assert.equal(body, '# Doc\n');
    assert.deepEqual(threads.map((x) => [x.id, x.comments.length]), [['t1', 2], ['t2', 1]]);
    assert.equal(mergeDocuments(merged.mine, theirs).added, 0);
    assert.equal(mergeDocuments(mine, theirs.replace('# Doc', '# Changed')).sameBody, false);
    const escaped = joinComments('x', [t('t', [{ body: '--> <b>' }])]);
    assert.ok(!escaped.includes('--> <b>') && escaped.includes('--\\u003e \\u003cb\\u003e'));
});
