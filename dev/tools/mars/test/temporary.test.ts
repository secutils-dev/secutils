import assert from 'node:assert/strict';
import test from 'node:test';
import { MemoryStore } from '../src/store/memory.ts';
import {
    MAX_AUTOMATIC_SOURCE,
    MAX_TEMPORARY_FRAGMENT,
    TemporaryRoute,
    temporaryFragment,
    type TemporaryLinkStatus,
} from '../src/share/temporary.ts';
import {
    decodeState,
    encodeState,
    parseBundle,
    sealBundle,
    openSealed,
    type SharedDoc,
    type Bundle,
} from '../src/share/codec.ts';

const doc: SharedDoc = {
    path: [],
    name: 'Temporary Ü',
    originId: 'temporary-origin',
    text: '---\r\ntitle: Temporary\r\n---\r\n\r\n# Temporary ✓\r\n\r\n<!-- su-table {"v":1,"cols":[0.3,0.7]} -->\r\n| A | B |\r\n| --- | --- |\r\n\r\n<!-- su-comments\r\n[]\r\n-->\r\n',
    readingWidth: 1100,
};

test('memory storage clones values and separates documents, side data, and settings', () => {
    const memory = new MemoryStore();
    const node = { id: 'one', name: 'Original' };
    memory.put('nodes', 'one', node);
    memory.put('docs', 'one', doc.text);
    memory.put('docs', 'opened:one', doc.text);
    memory.put('docs', 'vec:one', new Float32Array([0.25, 0.75]));
    memory.put('meta', 'lastDoc', 'one');
    node.name = 'Changed';
    assert.equal(memory.get<{ name: string }>('nodes', 'one')?.name, 'Original');
    assert.equal(memory.get('docs', 'one'), doc.text);
    assert.deepEqual(memory.all('docs', 'opened:'), [doc.text]);
    assert.deepEqual(memory.get('docs', 'vec:one'), new Float32Array([0.25, 0.75]));
    memory.clearPrefix('docs', 'vec:');
    assert.equal(memory.get('docs', 'vec:one'), undefined);
    assert.equal(memory.get('meta', 'lastDoc'), 'one');
    assert.equal(new MemoryStore().get('docs', 'one'), undefined);
});

test('temporary fragments retain exact source, names, origins, reading width, and the mode flag', async () => {
    const hash = await temporaryFragment(doc),
        params = new URLSearchParams(hash.slice(1));
    assert.equal(params.get('t'), '1');
    assert.equal(params.get('name'), doc.name);
    assert.equal(params.get('o'), doc.originId);
    assert.equal(params.get('w'), '1100');
    assert.equal(await decodeState(params.get('md')!), doc.text);
});

test('only the newest temporary document may replace the address bar', async () => {
    const writes: string[] = [],
        states: TemporaryLinkStatus[] = [];
    let release!: (text: string) => void;
    const pending = new Promise<string>((resolve) => {
        release = resolve;
    });
    const route = new TemporaryRoute(
        (hash) => writes.push(hash),
        (state) => states.push(state),
        (text) => (text === doc.text ? pending : encodeState(text)),
    );
    const old = route.update(doc);
    await route.update({ ...doc, text: '# Newest' });
    release(await encodeState(doc.text));
    await old;
    assert.equal(writes.length, 1);
    assert.equal(await decodeState(new URLSearchParams(writes[0].slice(1)).get('md')!), '# Newest');
    assert.equal(states.at(-1), 'ready');
});

test('closing or navigating away cancels pending temporary state encoding', async () => {
    const writes: string[] = [];
    let release!: (text: string) => void;
    const pending = new Promise<string>((resolve) => {
        release = resolve;
    });
    const route = new TemporaryRoute(
        (hash) => writes.push(hash),
        () => {},
        () => pending,
    );
    const old = route.update(doc);
    route.cancel();
    release('encoded');
    await old;
    assert.deepEqual(writes, []);
    await route.update(null);
    assert.deepEqual(writes, ['#t=1']);
});

test('oversized or failed URL state never leaves a stale saved document in the address bar', async () => {
    for (const encode of [
        async () => 'A'.repeat(MAX_TEMPORARY_FRAGMENT),
        async () => {
            throw new Error('Encoding failed');
        },
    ]) {
        const writes: string[] = [],
            states: TemporaryLinkStatus[] = [];
        const route = new TemporaryRoute(
            (hash) => writes.push(hash),
            (state) => states.push(state),
            encode,
        );
        await route.update(doc);
        assert.deepEqual(writes, ['#t=1']);
        assert.ok(['too-large', 'failed'].includes(states.at(-1)!));
    }
});

test('encrypted temporary shares retain their mode while malformed mode hints are ignored', async () => {
    const bundle: Bundle = { v: 1, kind: 'doc', name: doc.name, docs: [doc], temporary: true };
    const sealed = await sealBundle(bundle);
    assert.deepEqual(await openSealed(sealed.secret, sealed.blob), bundle);
    for (const temporary of [false, 'true', 1, null])
        assert.equal(parseBundle(JSON.stringify({ ...bundle, temporary }))?.temporary, undefined);
});

test('very large sources skip automatic compression and blocked history never breaks the reader', async () => {
    let compressed = false;
    const states: TemporaryLinkStatus[] = [];
    const route = new TemporaryRoute(
        () => {},
        (state) => states.push(state),
        async () => {
            compressed = true;
            return '';
        },
    );
    await route.update({ ...doc, text: 'A'.repeat(MAX_AUTOMATIC_SOURCE + 1) });
    assert.equal(compressed, false);
    assert.equal(states.at(-1), 'too-large');
    const blocked = new TemporaryRoute(
        () => {
            throw new Error('History disabled');
        },
        (state) => states.push(state),
    );
    await blocked.update(doc);
    assert.equal(states.at(-1), 'failed');
});
