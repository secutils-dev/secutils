import assert from 'node:assert/strict';
import { test } from 'node:test';
import { FeatureFlags, flagEnabled, parseFlags } from '../src/platform/flags.ts';

const environment = (search = '', saved: string | null = null, legacy = false) => {
    const state = { saved, legacy, consumed: false, change: (_value: string | null) => {} };
    const flags = new FeatureFlags({ read: () => state.saved, write: value => { state.saved = value; }, legacy: () => { const old = state.legacy; state.legacy = false; return old; }, query: () => new URLSearchParams(search), consume: () => { state.consumed = true; }, listen: changed => { state.change = changed; } });
    return { flags, state };
};
test('global rules, feature rules, and scoped opt-outs use one generic matcher', () => {
    const rules = parseFlags('debug,-debug:sync,experimental:live_links,debug:search');
    assert.equal(flagEnabled(rules, 'debug', 'search'), true);
    assert.equal(flagEnabled(rules, 'debug', 'sync'), false);
    assert.equal(flagEnabled(rules, 'experimental', 'live_links'), true);
    assert.equal(flagEnabled(rules, 'experimental', 'sync'), false);
    assert.equal(parseFlags('token,debug:<script>,private:secret,debug:ok,debug:ok').size, 1);
});
test('canonical flags replace the whole profile setting and consume their URL parameters', () => {
    const { flags, state } = environment('flags=debug:sync,experimental:live_links', 'debug,experimental');
    assert.deepEqual(flags.list(), ['debug:sync', 'experimental:live_links']);
    assert.equal(state.saved, 'debug:sync,experimental:live_links'); assert.equal(state.consumed, true);
    assert.equal(flags.enabled('debug', 'other'), false);
});
test('repeated category parameters support multiple features and explicit category Off', () => {
    const first = environment('debug=sync&debug=live&experimental=sync&experimental=live_links');
    assert.deepEqual(first.flags.list(), ['debug:live', 'debug:sync', 'experimental:live_links', 'experimental:sync']);
    const second = environment('debug=off', first.state.saved);
    assert.equal(second.flags.any('debug'), false); assert.equal(second.flags.enabled('experimental', 'sync'), true);
    assert.equal(environment('experimental=off', first.state.saved).flags.any('experimental'), false);
});
test('generic enable, disable, clear, and cross-tab updates take effect immediately', () => {
    const { flags, state } = environment();
    flags.enable('debug'); flags.disable('debug', 'sync');
    assert.equal(flags.enabled('debug', 'sync'), false); assert.equal(flags.enabled('debug', 'live_links'), true);
    flags.enable('experimental', 'sync'); flags.disable('debug');
    assert.equal(flags.any('debug'), false); assert.equal(flags.enabled('experimental', 'sync'), true);
    flags.clear(); assert.deepEqual(flags.list(), []); assert.equal(state.saved, '');
    state.change('debug:sync'); assert.equal(flags.enabled('debug', 'sync'), true);
    state.change(''); assert.equal(flags.any('debug'), false);
});
test('old Sync flags migrate once and cannot resurrect after a generic Off', () => {
    const first = environment('', null, true); assert.deepEqual(first.flags.list(), ['debug:sync']);
    first.flags.clear();
    const second = environment('', first.state.saved, true); assert.deepEqual(second.flags.list(), []);
    const queryOff = environment('flags=off', 'debug,experimental', true); assert.deepEqual(queryOff.flags.list(), []);
    assert.equal(queryOff.state.saved, '');
});
test('an explicit Off remains effective even if browser storage cannot be changed', () => {
    const flags = new FeatureFlags({ read: () => 'debug,experimental', write: () => { throw new DOMException('Denied', 'SecurityError'); }, legacy: () => true, query: () => new URLSearchParams('flags=off'), consume: () => {} });
    assert.deepEqual(flags.list(), []); assert.equal(flags.any('debug'), false); assert.equal(flags.any('experimental'), false);
});
