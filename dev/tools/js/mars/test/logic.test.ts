// Logic checks for the pure modules: `npm test` (Node >= 22, type stripping).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { decodeState, encodeState, openSealed, parseBundle, sealBundle, type Bundle } from '../src/share/codec.ts';
import { joinComments, mergeDocuments, splitComments, type Thread } from '../src/share/merge.ts';
import { canMove, childrenOf, exportPaths, moveNode, type TreeNode } from '../src/tree/model.ts';
import { diffBlocks, diffWords } from '../src/doc/diff.ts';
import { matchScore, textOverlap } from '../src/share/match.ts';
import { chunkDoc, embeddable, nudgeDue, searchBody, snippet, threadChunks } from '../src/search/chunks.ts';
import { createEncoder, indexChunks, MODELS, onnxFile, placement, rank, toVectors } from '../src/search/models.ts';

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

test('a merge that takes the shared body takes its anchors too', () => {
    const c = [{ id: 'c1', author: 'A', at: '2026-01-01', body: 'one' }];
    const mine = joinComments('Old words.\n', [{ id: 't1', quote: 'Old words', comments: c }]);
    const theirs = joinComments('New words.\n', [{ id: 't1', quote: 'New words', prefix: '', comments: [...c, { id: 'c2', author: 'Agent', at: '2026-01-02', body: 'Rewrote it.' }] }]);
    const merged = mergeDocuments(mine, theirs);
    assert.equal(splitComments(merged.mine).threads[0].quote, 'Old words');
    const taken = splitComments(merged.theirs).threads[0];
    assert.equal(taken.quote, 'New words');
    assert.equal(taken.comments.length, 2);
});

test('block diff pairs edits, additions, and removals in order', () => {
    assert.deepEqual(diffBlocks(['# A', 'one', 'two', 'three'], ['# A', 'one!', 'three', 'four']), [
        { kind: 'edited', old: 1, cur: 1 },
        { kind: 'removed', old: 2, cur: 2 },
        { kind: 'added', cur: 3 },
    ]);
    assert.deepEqual(diffBlocks(['a\n', 'b'], ['a', 'b\n\n']), []);
    assert.deepEqual(diffBlocks(['a', 'b'], []), [{ kind: 'removed', old: 0, cur: 0 }, { kind: 'removed', old: 1, cur: 0 }]);
    // Past the LCS ceiling, a whole gap counts as one replacement.
    const big = Array.from({ length: 2100 }, (_, i) => `p${i}`);
    const changed = diffBlocks(['head', ...big, 'tail'], ['head', ...big.slice(1), 'x', 'y', 'tail']);
    assert.ok(changed.every((c) => c.kind === 'edited' || c.kind === 'added'));
    assert.equal(changed.length, 2101);
});

test('an updated copy matches by threads, or by name or title plus shared text', () => {
    const doc = '# OLS replies\n\nFirst point stays.\nSecond point stays.\nThird point stays.\n';
    const revised = `${doc}A new fourth point.\n`;
    assert.ok(textOverlap(doc, revised) > 0.7);
    assert.ok(matchScore({ name: 'test.md', text: revised }, { name: 'test v1', text: doc }) > 0.5);
    assert.ok(matchScore({ name: 'other.md', text: '# Other\n\nUnrelated words here.\n' }, { name: 'test v1', text: doc }) === 0);
    const thread = [{ id: 't1', quote: 'x', comments: [{ body: 'c' }] }];
    assert.equal(matchScore({ name: 'a', text: joinComments('Totally new.', thread) }, { name: 'b', text: joinComments('Old.', thread) }), 1);
    // A shared name alone is not enough.
    assert.equal(matchScore({ name: 'README.md', text: '# A\n\nOne repo.\n' }, { name: 'README', text: '# B\n\nAnother repo.\n' }), 0);
});

test('documents chunk by paragraph with heading context, skipping data fences', () => {
    const body = searchBody('---\ntitle: T\n---\n# Intro\n\nHello **world**.\n\n```stl\nsolid x\n```\n\n## Next\n\nMore [text](https://x).\n\n<!-- su-comments\n[]\n-->\n');
    const chunks = chunkDoc(body);
    assert.deepEqual(chunks.map((c) => c.text), ['Intro Hello world.', 'Next More text.']);
    assert.equal(body.slice(chunks[1].start, chunks[1].start + 7), '## Next');
    assert.equal(snippet('The cache **needs a TTL** before launch.', 'ttl', 4), '…needs a TTL before…');
    assert.equal(snippet('nothing', 'ttl'), null);
});

test('review threads become searchable chunks, and one-word passages get no vector', () => {
    const text = joinComments('# Plan\n\nThe cache needs a TTL.\n', [
        { id: 't1', quote: 'needs a TTL', comments: [{ body: 'Which **TTL**?' }, { body: 'Use 15 minutes.' }] },
        { id: 't2', quote: 'Plan', comments: [{ body: 'ok' }] },
    ]);
    const body = searchBody(text);
    assert.deepEqual(threadChunks(text, body), [
        { start: body.indexOf('needs a TTL'), text: 'Comment on “needs a TTL”: Which TTL? Use 15 minutes.', embed: 'Which TTL? Use 15 minutes.' },
        { start: body.indexOf('Plan'), text: 'Comment on “Plan”: ok', embed: 'ok' },
    ]);
    assert.equal(threadChunks(text, body, 3).length, 1);
    assert.equal(embeddable({ start: 0, text: 'Dog' }), false);
    assert.equal(embeddable({ start: 0, text: 'The cache expires' }), true);
});

test('embeddings are truncated and normalized, and ranking keeps each document\'s best passage', () => {
    const m = { ...MODELS.best, dims: 2 };
    const [a, b] = toVectors(m, 3, new Float32Array([3, 4, 9, 0, 2, 9]), 2);
    assert.deepEqual([...a].map((x) => +x.toFixed(3)), [0.6, 0.8]);
    assert.deepEqual([...b], [0, 1]);
    const docs = [{ id: 'x', chunks: [{ start: 0, text: 'p', v: a }, { start: 5, text: 'q', v: b }] }, { id: 'y', chunks: [{ start: 0, text: 'r', v: a }] }];
    assert.deepEqual(rank(docs, new Float32Array([0, 1])).map((h) => [h.id, h.start, +h.score.toFixed(2)]), [['x', 5, 1], ['y', 0, 0.8]]);
    assert.deepEqual(rank(docs, new Float32Array([0, 1]), 0.9).map((h) => h.id), ['x']);
});

test('models load float16 weights on WebGPU with shader-f16, and Best quality needs a GPU', () => {
    const at = (key: keyof typeof MODELS, ok: boolean, f16: boolean) => placement(MODELS[key], { ok, f16 })?.device + '@' + placement(MODELS[key], { ok, f16 })?.dtype;
    assert.equal(at('fast', true, true), 'webgpu@fp16');
    assert.equal(at('fast', true, false), 'wasm@q8');
    assert.equal(at('fast', false, false), 'wasm@q8');
    assert.equal(at('best', true, true), 'webgpu@q4f16');
    assert.equal(at('best', true, false), 'webgpu@q4');
    assert.equal(placement(MODELS.best, { ok: false, f16: false }), null);
    assert.equal(onnxFile(MODELS.fast, 'q8'), 'model_quantized.onnx');
});

test('the encoder runs from its own source text, loads text only, and pools when the model does not', async () => {
    const calls: Record<string, unknown> = {};
    const tf = {
        AutoConfig: { from_pretrained: async () => ({ vision_config: {}, audio_config: {} }) },
        AutoTokenizer: { from_pretrained: async () => (texts: string[]) => ({ attention_mask: texts.length }) },
        AutoModel: { from_pretrained: async (_: string, opts: Record<string, unknown>) => { calls.opts = opts; return async () => ({ last_hidden_state: 'h' }); } },
        mean_pooling: (h: string, mask: number) => ({ dims: [mask, 1], data: new Float32Array([1]), pooled: h }),
    };
    // The worker evaluates the function's source, so it must not depend on module scope.
    const encoder = new Function(`return ${createEncoder.toString()}`)() as typeof createEncoder;
    const encode = await encoder(tf, { model: 'm', file: 'model', dtype: 'q8' }, 'cpu');
    assert.deepEqual((calls.opts as { config: unknown }).config, { vision_config: null, audio_config: null });
    assert.equal(((await encode(['a'])) as unknown as { pooled: string }).pooled, 'h');
    assert.equal(indexChunks('# T\n\nOne two three.\n\nDog\n').length, 1);
});

test('the smart-search suggestion appears at 6, 20, 50, then every 10 documents', () => {
    assert.equal(nudgeDue(5, 0), null);
    assert.equal(nudgeDue(6, 0), 6);
    assert.equal(nudgeDue(19, 6), null);
    assert.equal(nudgeDue(37, 6), 20);
    assert.equal(nudgeDue(55, 20), 50);
    assert.equal(nudgeDue(59, 50), null);
    assert.equal(nudgeDue(60, 50), 60);
    assert.equal(nudgeDue(84, 60), 80);
});

test('word diff returns inserted ranges of the new text', () => {
    const next = 'Queries must fail closed when OLS cannot apply.';
    const ranges = diffWords('Queries return warnings when OLS cannot apply.', next)!;
    assert.deepEqual(ranges.map(([s, e]) => next.slice(s, e)), ['must fail closed']);
    assert.deepEqual(diffWords('same text', 'same  text'), []);
    assert.equal(diffWords('x '.repeat(1100), 'y'), null);
});
