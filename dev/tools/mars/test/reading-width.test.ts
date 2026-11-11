import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
const md = readFileSync(new URL('../../md/index.html', import.meta.url), 'utf8');
const section = (text: string) =>
    text.slice(
        text.indexOf('const READING_WIDTH_MIN'),
        text.indexOf("window.addEventListener('pageshow', restoreReadingWidth);") +
            "window.addEventListener('pageshow', restoreReadingWidth);".length,
    );

function setup(saved: string | null = null, denied = false) {
    const styles = new Map<string, string>();
    const callbacks = new Map<string, () => void>();
    const input = (id: string, value: string) => ({
        value,
        hidden: true,
        options: ['800', '1100', 'full', 'custom'].map((value) => ({ value })),
        focus() {},
        addEventListener(event: string, fn: () => void) {
            callbacks.set(`${id}:${event}`, fn);
        },
    });
    const els = {
        optReadingWidth: input('width', '800'),
        optReadingCustom: input('custom', '800'),
        optReadingCustomWrap: { hidden: true },
    };
    runInNewContext(section(viewer), {
        els,
        document: {
            documentElement: { style: { setProperty: (key: string, value: string) => styles.set(key, value) } },
        },
        window: { addEventListener: (event: string, fn: () => void) => callbacks.set(event, fn) },
        localStorage: {
            getItem() {
                if (denied) throw new Error('Storage blocked');
                return saved;
            },
            setItem(_key: string, value: string) {
                if (denied) throw new Error('Storage blocked');
                saved = value;
            },
        },
    });
    return { els, styles, callbacks, saved: () => saved };
}

test('reading width restoration matches Markdown Preview', () => {
    assert.equal(section(viewer), section(md).replace("    if (viewMode === 'html') renderHtmlPreview();\n", ''));
});

test('saved full and custom widths synchronize controls and both layout properties at startup', () => {
    const full = setup(JSON.stringify({ width: 'full', customWidth: 980 }));
    assert.equal(full.els.optReadingWidth.value, 'full');
    assert.equal(full.styles.get('--su-reading-width'), 'none');
    assert.equal(full.styles.get('--su-reading-track'), '1fr');
    assert.equal(full.els.optReadingCustom.value, '980');
    assert.equal(full.els.optReadingCustomWrap.hidden, true);
    const custom = setup(JSON.stringify({ width: 980, customWidth: 800 }));
    assert.equal(custom.els.optReadingWidth.value, 'custom');
    assert.equal(custom.els.optReadingCustom.value, '980');
    assert.equal(custom.els.optReadingCustomWrap.hidden, false);
    assert.equal(custom.styles.get('--su-reading-width'), '980px');
    assert.equal(custom.styles.get('--su-reading-track'), '980px');
});

test('missing, malformed, and invalid saved widths keep the default layout usable', () => {
    for (const saved of [null, '{broken', 'null', '{}', JSON.stringify({ width: 2401, customWidth: 'full' })]) {
        const state = setup(saved);
        assert.equal(state.els.optReadingWidth.value, '800');
        assert.equal(state.els.optReadingCustom.value, '800');
        assert.equal(state.styles.get('--su-reading-width'), '800px');
    }
});

test('valid width changes save the selection and last custom width; invalid inputs do not', () => {
    const state = setup();
    state.els.optReadingWidth.value = 'custom';
    state.callbacks.get('width:change')!();
    state.els.optReadingCustom.value = '980';
    state.callbacks.get('custom:input')!();
    assert.deepEqual(JSON.parse(state.saved()!), { width: 980, customWidth: 980 });
    for (const invalid of ['', '319', '2401', '800.5']) {
        state.els.optReadingCustom.value = invalid;
        state.callbacks.get('custom:input')!();
        assert.deepEqual(JSON.parse(state.saved()!), { width: 980, customWidth: 980 });
    }
    state.callbacks.get('custom:change')!();
    assert.equal(state.els.optReadingCustom.value, '980');
    state.els.optReadingWidth.value = 'full';
    state.callbacks.get('width:change')!();
    assert.deepEqual(JSON.parse(state.saved()!), { width: 'full', customWidth: 980 });
});

test('blocked storage still applies changes and native form restoration on pageshow', () => {
    const state = setup(null, true);
    state.els.optReadingWidth.value = 'full';
    state.callbacks.get('width:change')!();
    assert.equal(state.styles.get('--su-reading-width'), 'none');
    state.els.optReadingWidth.value = 'custom';
    state.els.optReadingCustom.value = '980';
    state.callbacks.get('pageshow')!();
    assert.equal(state.styles.get('--su-reading-width'), '980px');
    assert.equal(state.els.optReadingCustomWrap.hidden, false);
});
