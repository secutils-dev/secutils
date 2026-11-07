import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';
import { validWidth } from '../src/export/settings.ts';
import { encodeState, decodeState, parseBundle, sealBundle, openSealed, type Bundle } from '../src/share/codec.ts';

const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
const start = viewer.indexOf('// ---- Shared reading layout (MARS only) ----');
const preference = viewer.slice(viewer.indexOf('const READING_WIDTH_MIN'), start);
const layout = viewer.slice(start, viewer.indexOf('// ---- File open', start)).replace(/^export /gm, '');

function setup(initial: unknown = null, nativeWidth = '800') {
    let saved = initial === null ? null : JSON.stringify(initial);
    const styles = new Map<string, string>(), events = new Map<string, (() => void)[]>(), widths: unknown[] = [];
    const on = (key: string, callback: () => void) => events.set(key, [...events.get(key) ?? [], callback]);
    const input = (id: string, value: string) => ({ value, options: ['800', '1100', 'full', 'custom'].map((value) => ({ value })), focus() {}, addEventListener: (type: string, fn: () => void) => on(`${id}:${type}`, fn) });
    const els = { optReadingWidth: input('width', nativeWidth), optReadingCustom: input('custom', '800'), optReadingCustomWrap: { hidden: true } };
    const api = runInNewContext(`${preference}\n${layout}\n({ set: setDocumentReadingWidth, defaultWidth: defaultReadingWidth })`, {
        els, validWidth, toast() {},
        host: { readingWidthChanged: async (width: unknown) => { widths.push(width); } },
        document: { documentElement: { style: { setProperty: (key: string, value: string) => styles.set(key, value) } } },
        window: { addEventListener: (type: string, fn: () => void) => on(type, fn) },
        localStorage: { getItem: () => saved, setItem: (_: string, value: string) => { saved = value; } },
    });
    return { api, els, styles, widths, fire: (key: string) => { for (const fn of events.get(key) ?? []) fn(); }, saved: () => saved };
}

test('shared reading widths synchronize controls without changing the browser preference', () => {
    const state = setup();
    for (const width of [1100, 980, 'full']) {
        state.api.set(width);
        assert.equal(state.styles.get('--su-reading-width'), width === 'full' ? 'none' : `${width}px`);
        assert.equal(state.styles.get('--su-reading-track'), width === 'full' ? '1fr' : `${width}px`);
        assert.equal(state.api.defaultWidth(), 800);
        assert.equal(state.saved(), null);
    }
    state.api.set();
    assert.equal(state.styles.get('--su-reading-width'), '800px');
    assert.equal(state.els.optReadingWidth.value, '800');
    assert.equal(state.els.optReadingCustom.value, '800');
});

test('document switching and pageshow retain separate shared and preferred widths', () => {
    const state = setup({ width: 'full', customWidth: 960 });
    state.api.set(1100);
    state.fire('pageshow');
    assert.equal(state.styles.get('--su-reading-width'), '1100px');
    assert.equal(state.api.defaultWidth(), 'full');
    state.api.set();
    assert.equal(state.els.optReadingWidth.value, 'full');
    assert.equal(state.els.optReadingCustom.value, '960');
    assert.deepEqual(JSON.parse(state.saved()!), { width: 'full', customWidth: 960 });
    for (const invalid of [0, 319, 2401, 800.5, '1100', '<script>', null]) {
        state.api.set(invalid);
        assert.equal(state.styles.get('--su-reading-width'), 'none');
    }
});

test('native form restoration before opening a shared document cannot become the default', () => {
    const state = setup(null, '1100');
    state.fire('pageshow');
    state.api.set(1100);
    assert.equal(state.api.defaultWidth(), 800);
    state.api.set();
    assert.equal(state.styles.get('--su-reading-width'), '800px');
    assert.equal(state.saved(), null);
});

test('explicit valid changes update the shared document and the reader preference', () => {
    const state = setup();
    state.api.set(1100);
    state.els.optReadingWidth.value = 'custom';
    state.fire('width:change');
    state.els.optReadingCustom.value = '980';
    state.fire('custom:input');
    assert.equal(state.widths.at(-1), 980);
    assert.equal(state.api.defaultWidth(), 980);
    for (const invalid of ['', '319', '2401', '900.5']) {
        state.els.optReadingCustom.value = invalid;
        const calls = state.widths.length;
        state.fire('custom:input');
        assert.equal(state.widths.length, calls);
    }
    state.api.set();
    assert.equal(state.styles.get('--su-reading-width'), '980px');
    assert.deepEqual(JSON.parse(state.saved()!), { width: 980, customWidth: 980 });
});

test('bundle encoding and encryption preserve valid reading widths and ignore invalid layout', async () => {
    for (const readingWidth of [800, 1100, 980, 'full'] as const) {
        const bundle: Bundle = { v: 1, kind: 'doc', name: 'Layout', docs: [{ path: [], name: 'Layout', originId: 'layout', text: '# Layout', readingWidth }] };
        assert.deepEqual(parseBundle((await decodeState(await encodeState(JSON.stringify(bundle))))!), bundle);
        const sealed = await sealBundle(bundle);
        assert.deepEqual(await openSealed(sealed.secret, sealed.blob), bundle);
    }
    for (const readingWidth of [undefined, '1100', 319, 2401, '<style>', null]) {
        const parsed = parseBundle(JSON.stringify({ v: 1, kind: 'doc', name: 'Layout', docs: [{ path: [], name: 'Layout', originId: 'layout', text: '# Layout', readingWidth }] }));
        assert.equal(parsed?.docs[0].text, '# Layout');
        assert.equal(parsed?.docs[0].readingWidth, undefined);
    }
});

test('document and folder shares use the current, stored, or preferred reading width', async () => {
    const source = readFileSync(new URL('../src/share/share.ts', import.meta.url), 'utf8');
    const code = source.slice(source.indexOf('async function bundleFor'), source.indexOf('// ---- Short links'));
    const nodes = [
        { id: 'folder', kind: 'folder', name: 'Folder' },
        { id: 'current', kind: 'doc', name: 'Current' },
        { id: 'stored', kind: 'doc', name: 'Stored', readingWidth: 'full' },
        { id: 'default', kind: 'doc', name: 'Default' },
    ];
    const api = runInNewContext(`${transformSync(code, { loader: 'ts' }).code}\n({ bundleFor, fullLink })`, {
        app: { node: (id: string) => nodes.find((n) => n.id === id), state: { nodes, currentId: 'current' }, docText: async () => '# Layout' },
        viewer: { exportReadingWidth: () => 980, defaultReadingWidth: () => 1100 },
        descendants: () => nodes.slice(1), ancestors: () => [],
        validWidth, URLSearchParams, encodeState, base: () => 'https://mars.example/',
    });
    for (const [id, expected] of [['current', 980], ['stored', 'full'], ['default', 1100]]) {
        const bundle = await api.bundleFor(id);
        assert.equal(bundle.docs[0].readingWidth, expected);
        assert.equal(new URLSearchParams(new URL(await api.fullLink(bundle)).hash.slice(1)).get('w'), String(expected));
    }
    const folder = await api.bundleFor('folder');
    assert.equal(JSON.stringify(folder.docs.map((d: { readingWidth: unknown }) => d.readingWidth)), '[980,"full",1100]');
    const params = new URLSearchParams(new URL(await api.fullLink(folder)).hash.slice(1));
    assert.deepEqual(parseBundle((await decodeState(params.get('b')!))!)?.docs.map((doc) => doc.readingWidth), [980, 'full', 1100]);
});
