import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { transformSync } from 'esbuild';

const sidebar = readFileSync(new URL('../src/sidebar/sidebar.ts', import.meta.url), 'utf8');
// Evaluate the actual layout handlers without importing the document tree and viewer.
const source = (
    sidebar.slice(sidebar.indexOf('const NARROW ='), sidebar.indexOf('// The node whose row')) +
    sidebar.slice(
        sidebar.indexOf('// ---- Collapse, overlay, and width ----'),
        sidebar.indexOf('/** Applies a sidebar width'),
    )
).replace(/^export /gm, '');

function setup(narrow = false, saved: string | null = null, storageBlocked = false) {
    const classes = new Set<string>();
    const events = new Map<string, () => void>();
    const attributes = new Map<string, string>();
    const backdrop = {
        hidden: true,
        addEventListener: (type: string, fn: () => void) => events.set(`backdrop:${type}`, fn),
    };
    const media = { matches: narrow, addEventListener: (_: string, fn: () => void) => events.set('resize', fn) };
    const api = runInNewContext(`${transformSync(source, { loader: 'ts' }).code}\n({ openSidebar, closeOverlay })`, {
        matchMedia: () => media,
        document: {
            body: {
                classList: {
                    toggle: (name: string, enabled: boolean) => (enabled ? classes.add(name) : classes.delete(name)),
                    contains: (name: string) => classes.has(name),
                },
            },
        },
        $: (id: string) =>
            id === 'sbBackdrop'
                ? backdrop
                : {
                      setAttribute: (name: string, value: string) => attributes.set(name, value),
                      addEventListener: (type: string, fn: () => void) => events.set(`toggle:${type}`, fn),
                  },
        localStorage: {
            getItem: () => {
                if (storageBlocked) throw new Error('Storage unavailable');
                return saved;
            },
            setItem: (_: string, value: string) => {
                if (storageBlocked) throw new Error('Storage unavailable');
                saved = value;
            },
        },
    });
    return {
        api,
        saved: () => saved,
        toggle: () => events.get('toggle:click')!(),
        dismiss: () => events.get('backdrop:click')!(),
        resize: (narrow: boolean) => {
            media.matches = narrow;
            events.get('resize')!();
        },
        assertCollapsed: (collapsed: boolean) => {
            assert.equal(classes.has('sb-collapsed'), collapsed);
            assert.equal(attributes.get('aria-expanded'), String(!collapsed));
            assert.equal(backdrop.hidden, collapsed || !media.matches);
        },
    };
}

test('an explicit docked hide survives repeated breakpoint crossings', () => {
    const state = setup();
    state.assertCollapsed(false);
    state.toggle();
    assert.equal(state.saved(), '1');
    for (const narrow of [false, true, false, true, false]) {
        state.resize(narrow);
        state.assertCollapsed(true);
    }
});

test('reopening a previously hidden docked sidebar replaces the startup preference', () => {
    const state = setup(false, '1');
    state.assertCollapsed(true);
    state.api.openSidebar();
    assert.equal(state.saved(), '');
    for (const narrow of [true, false, true, false]) {
        state.resize(narrow);
        state.assertCollapsed(narrow);
    }
});

test('temporary overlay actions never overwrite the docked preference', () => {
    for (const saved of [null, '1']) {
        const state = setup(true, saved);
        state.assertCollapsed(true);
        for (const close of [state.toggle, state.dismiss, state.api.closeOverlay]) {
            state.api.openSidebar();
            state.assertCollapsed(false);
            close();
            state.assertCollapsed(true);
            assert.equal(state.saved(), saved);
        }
        state.resize(false);
        state.assertCollapsed(saved === '1');
    }
});

test('the latest docked choice survives resize even when storage is unavailable', () => {
    const state = setup(false, null, true);
    for (const collapsed of [true, false, true]) {
        state.toggle();
        state.resize(true);
        state.assertCollapsed(true);
        state.resize(false);
        state.assertCollapsed(collapsed);
    }
});
